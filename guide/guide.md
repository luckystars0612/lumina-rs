# ARCHITECTURE GUIDE: LUMINA-RS (CONTEXT-AWARE PARTICLE ENGINE)

## 1. PROJECT PHILOSOPHY: THE "DOUBLE-CHECK" SYSTEM
Lumina-RS is a generative engine that synchronizes two layers of data to ensure particles "exist within" the scene rather than just floating over it:
- **Layer 1: Intent (LLM-Driven):** Global artistic parameters (Preset, Base Color, Speed).
- **Layer 2: Context (Image-Driven):** Local physical constraints extracted from the background (Luminance, Color Sampling, Depth).

## 2. TECHNICAL STACK
- **Language:** Rust (Stable).
- **Graphics API:** WGPU (Headless mode for GTX 1070).
- **Encoding:** FFmpeg (via `stdin`) with `h264_nvenc` support.
- **Math:** `glam` for vectors, `noise-rs` for Simplex/Curl noise.

## 3. DATA STRUCTURES & INPUTS

### 3.1. Gemini JSON Input (The Intent)
Agent must implement a parser for this schema:
{
  "image_prompt": "Flux-1-schnell prompt description",
  "overlay_config": {
    "preset": "rain | snow | fireflies | sun_dust | petals | embers",
    "params": {
      "base_color_hex": "#RRGGBB",
      "velocity_scale": [f32, f32],
      "density": f32,
      "turbulence": f32,
      "flicker": f32,
      "size_range": [f32, f32],
      "sway_intensity": f32,
      "buoyancy_force": f32
    },
    "double_check_logic": "Descriptive string for behavior refinement"
  }
}

### 3.2. Background Image Analysis (The Context)
- **Luminance Map:** Create a Grayscale Texture where `Value = 0.299R + 0.587G + 0.114B`.
- **Spawn Probability:** Use the Luminance Map to bias particle spawning (High brightness = Higher spawn rate for dust/fireflies).

## 4. THE "DOUBLE-CHECK" EXECUTION (AGENT LOGIC)

### A. Contextual Spawn
- **Logic:** `SpawnProbability = textureSample(LuminanceMap, uv).r * config.density`.
- **Constraint:** In "Rain" preset, bypass this to allow uniform top-down spawning, but use the mask to dim particles in dark areas.

### B. Adaptive Coloring (Chameleon Effect)
- **Logic:** In Fragment Shader, `FinalColor = mix(BaseColor, BackgroundPixelColor * 1.5, 0.5)`.
- **Effect:** Particles adopt the local lighting of the Flux-generated image.

### C. Depth & Bokeh
- **Logic:** Assign random `z` [0.0 to 1.0].
- **Visuals:** Apply Gaussian Blur and Size scaling based on `z` to simulate Foreground and Background Bokeh.

## 5. PHYSICS & LOOPING

### 5.1. Seamless 10s Loop (600 Frames @ 60fps)
- **Math:** Map `time` to a circle: `x = cos(2π * t/10)`, `y = sin(2π * t/10)`.
- **Noise:** Sample 4D Simplex noise using `(pos.x, pos.y, x, y)`.
- **Result:** `Noise(t=0.0)` is mathematically identical to `Noise(t=10.0)`.

### 5.2. Preset Branches
The Engine uses the `preset` field to switch between physics kernels in the `compute.wgsl` shader.

| Preset Name | Physics Engine | Visual Shape | Description |
| :--- | :--- | :--- | :--- |
| **Normal** | `Basic Linear` | `Soft Circle` | Standard particles with constant velocity and no special behavior. Ideal for generic dust or simple floating motes. |
| **Rain** | `Linear + Accel` | `Stretched Line` | High-speed vertical movement with elongation based on velocity. |
| **Snow** | `Sway + Linear` | `Soft Circle` | Slow falling with a horizontal sine-wave oscillation (wobble). |
| **Fireflies** | `Curl Noise` | `Glow Circle` | Organic swirling motion with biological flickering (light pulse). |
| **Sun Dust** | `Brownian` | `Micro Pixel` | Ultra-slow, erratic movement. High reactivity to light beams. |
| **Embers** | `Fluid + Rising` | `Glow Point` | Upward rising particles with turbulence, fading from orange to ash-grey. |
| **Petals** | `Sway + Rotate` | `Ellipse` | Falling with horizontal sway and 2D rotation to simulate air resistance. |
| **Bubbles** | `Buoyancy` | `Ring SDF` | Slow upward movement with "wobble" and high transparency. |
## 6. EXPORT PIPELINE (FFMPEG)
Agent must pipe raw RGBA buffers to FFmpeg:
`ffmpeg -f rawvideo -pixel_format rgba -video_size 1920x1080 -framerate 60 -i - -c:v h264_nvenc -preset p7 -tune hq -rc vbr -cq 22 -pix_fmt yuv420p -movflags +faststart output.mp4`

## 7. AGENT IMPLEMENTATION TASKS
1. **Initialize WGPU Headless:** Target the GTX 1070.
2. **Pre-processor:** Create a compute shader for the Luminance Map.
3. **Storage Buffers:** Allocate GPU memory for 100,000 `Particle` structs.
4. **Physics Shader (`compute.wgsl`):** Implement the 10s circular loop and Preset branches.
5. **Render Shader (`render.wgsl`):** Implement SDF drawing, Additive Blending, and Background Sampling.
6. **Orchestrator:** Loop 600 frames, write Texture to `stdout`.

---
**Note to Agent:** Prioritize organic movement using Curl Noise. Avoid pure random jitter.