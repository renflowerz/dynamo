// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use opentelemetry::trace::{SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState};
use pyo3::prelude::*;
use pythonize::{depythonize, pythonize};
use tokio_stream::StreamExt;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use dynamo_llm::entrypoint::PrefillRoutedEngine;
use dynamo_llm::protocols::common::preprocessor::PreprocessedRequest;
use dynamo_runtime::pipeline::{AsyncEngineContextProvider, SingleIn};
use dynamo_runtime::protocols::annotated::Annotated as RsAnnotated;

use crate::to_pyerr;

#[pyclass]
pub struct RoutedEngine {
    inner: PrefillRoutedEngine,
}

impl RoutedEngine {
    pub fn new(inner: PrefillRoutedEngine) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl RoutedEngine {
    /// Send a preprocessed request through the Rust prefill-routed pipeline.
    #[pyo3(signature = (preprocessed, context=None))]
    fn generate<'p>(
        &self,
        py: Python<'p>,
        preprocessed: PyObject,
        context: Option<crate::context::Context>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let request: PreprocessedRequest = depythonize(preprocessed.bind(py)).map_err(to_pyerr)?;
        let request_context = if let Some(parent_context) = context.as_ref() {
            let parent_metadata = parent_context.metadata_snapshot();
            let parent_context = parent_context.inner();
            let child_context = SingleIn::with_id_and_metadata(
                request,
                parent_context.id().to_string(),
                parent_metadata,
            );
            let child_controller = child_context.context();
            parent_context.link_child(child_controller.clone());
            if parent_context.is_killed() {
                child_controller.kill();
            } else if parent_context.is_stopped() {
                child_controller.stop_generating();
            }
            child_context
        } else {
            SingleIn::new(request)
        };
        let inner = self.inner.clone();

        // Restore the caller's trace context across the Python boundary.
        //
        // When the frontend hands a request to a Python chat processor, the
        // Rust side captures its `DistributedTraceContext` (the identity of
        // the `http-request` span) into the PyContext (engine.rs:
        // `Context::new(ctx, get_distributed_tracing_context(), ..)`). The
        // Python processor then calls back into this method — but the future
        // below runs on a fresh tokio task where `Span::current()` is empty.
        // Without re-parenting, every downstream dispatch span
        // (`router.route_request` → worker `handle_payload` → engine spans)
        // becomes an orphan trace root: one request fans out into three
        // unrelated traces (http-request / prefill / decode) instead of one.
        //
        // Re-parent a `routed_engine.generate` span to the captured context
        // so the whole PD pipeline lands back on the request's trace.
        let dispatch_span = match context.as_ref().and_then(|c| c.trace_context()) {
            Some(tc) => {
                let span = tracing::info_span!(
                    "routed_engine.generate",
                    request_id = request_context.id()
                );
                if let (Ok(trace_id), Ok(span_id)) = (
                    TraceId::from_hex(&tc.trace_id),
                    SpanId::from_hex(&tc.span_id),
                ) {
                    let span_context = SpanContext::new(
                        trace_id,
                        span_id,
                        TraceFlags::SAMPLED,
                        true, // is_remote
                        TraceState::default(),
                    );
                    let _ = span.set_parent(
                        opentelemetry::Context::new().with_remote_span_context(span_context),
                    );
                }
                span
            }
            // No trace context on the PyContext (e.g. tests constructing a
            // bare Context, or tracing disabled): keep current behavior.
            None => tracing::Span::current(),
        };

        pyo3_async_runtimes::tokio::future_into_py(
            py,
            async move {
                let mut stream = inner.generate(request_context).await.map_err(to_pyerr)?;
                let task_context = stream.context();
                let (tx, rx) = tokio::sync::mpsc::channel::<RsAnnotated<PyObject>>(32);

                tokio::spawn(async move {
                    loop {
                        let response = tokio::select! {
                            _ = tx.closed() => {
                                task_context.stop_generating();
                                break;
                            }
                            response = stream.next() => response,
                        };

                        let Some(response) = response else {
                            break;
                        };

                        let py_response = Python::with_gil(|py| {
                            response.map_data(|data| {
                                pythonize(py, &data)
                                    .map(|obj| obj.unbind())
                                    .map_err(|e| format!("pythonize failed: {e}"))
                            })
                        });

                        if tx.send(py_response).await.is_err() {
                            task_context.stop_generating();
                            break;
                        }
                    }
                });

                Ok(crate::AsyncResponseStream::new(rx, true))
            }
            .instrument(dispatch_span),
        )
    }
}
