/*
 * SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

package operatorenv

import (
	"testing"

	configv1alpha1 "github.com/ai-dynamo/dynamo/deploy/operator/api/config/v1alpha1"
	"k8s.io/utils/ptr"
)

func TestDefaultOperatorConfigPreservesExplicitGPUDiscovery(t *testing.T) {
	t.Log("Create an operator configuration with GPU discovery explicitly enabled")
	config := &configv1alpha1.OperatorConfiguration{
		GPU: configv1alpha1.GPUConfiguration{
			DiscoveryEnabled: ptr.To(true),
		},
	}

	t.Log("Build the envtest operator configuration")
	got := defaultOperatorConfig(config)

	t.Log("Assert the explicit GPU discovery setting is preserved")
	if got.GPU.DiscoveryEnabled == nil || !*got.GPU.DiscoveryEnabled {
		t.Fatalf("GPU discovery enabled = %v, want true", got.GPU.DiscoveryEnabled)
	}
}
