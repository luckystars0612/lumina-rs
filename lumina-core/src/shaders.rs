//! WGSL Shader source code for particle physics and rendering

/// Compute shader for particle physics simulation
/// Handles: particle position updates, 10s seamless loop, preset-specific physics
pub const COMPUTE_SHADER: &str = r#"
// Lumina-RS Compute Shader
// Particle physics simulation with 6 presets

struct SimParams {
    delta_time: f32,
    time: f32,           // 0-10s loop time
    width: f32,
    height: f32,
    preset: u32,          // 0=CosmicDust, 1=Rain, 2=Snow, 3=Fireflies, 4=SunDust, 5=Embers
    density: f32,
    velocity_min: f32,
    velocity_max: f32,
    turbulence: f32,
    flicker_speed: f32,
    size_min: f32,
    size_max: f32,
    sway_intensity: f32,
    buoyancy_force: f32,
    // Snow-specific params
    fall_speed_mult: f32,
    fall_direction: f32,
    // Fireflies-specific
    visibility_ratio: f32,
    // Rain-specific params
    wind_direction: f32,   // degrees: 0=down, 90=right
    wind_strength: f32,     // 0.0-2.0
    gust_enabled: f32,     // 0 or 1
    gust_frequency: f32,   // 0.0-1.0
    gust_strength: f32,    // 1.0-3.0
    gust_duration: f32,    // seconds
    rain_type: u32,        // 0=drizzle, 1=normal, 2=heavy, 3=storm
    splash_enabled: f32,    // 0 or 1
    splash_velocity: f32,  // multiplier
}

struct Particle {
    position: vec3f,     // xy = screen pos, z = depth
    velocity: vec3f,     // current velocity
    seed: vec2f,         // random seed for variation
    lifetime: f32,       // particle lifetime (0-1)
    size: f32,           // particle size
    phase: f32,          // animation phase
}

@group(0) @binding(0) var<uniform> params: SimParams;
@group(0) @binding(1) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(2) var luminance_mask: texture_2d<f32>;
@group(0) @binding(3) var luminance_sampler: sampler;

// 4D Simplex noise for seamless looping
fn simplex4d(p: vec4f) -> f32 {
    let hash = dot(p, vec4f(12.9898, 78.233, 45.164, 94.673)) * 43758.5453;
    return fract(sin(hash) * 43758.5453);
}

// Seamless loop noise: lerp between two time-offset samples
fn loop_noise(pos: vec2f, t: f32, loop_duration: f32) -> f32 {
    let t1 = simplex4d(vec4f(pos.x * 0.1, pos.y * 0.1, t, 0.0));
    let t2 = simplex4d(vec4f(pos.x * 0.1, pos.y * 0.1, t - loop_duration, 0.0));
    let blend = t / loop_duration;
    return mix(t1, t2, blend);
}

// Curl noise for organic motion
fn curl_noise(pos: vec2f, t: f32) -> vec2f {
    let eps = 0.01;
    let n1 = loop_noise(pos + vec2f(eps, 0.0), t, 10.0);
    let n2 = loop_noise(pos - vec2f(eps, 0.0), t, 10.0);
    let n3 = loop_noise(pos + vec2f(0.0, eps), t, 10.0);
    let n4 = loop_noise(pos - vec2f(0.0, eps), t, 10.0);
    return vec2f((n1 - n2) / (2.0 * eps), (n3 - n4) / (2.0 * eps));
}

// Hash function for random values
fn hash(seed: vec2f) -> f32 {
    return fract(sin(dot(seed, vec2f(12.9898, 78.233))) * 43758.5453);
}

fn sample_luminance(uv: vec2f) -> f32 {
    let tex_size = vec2u(textureDimensions(luminance_mask));
    let tex_coord = vec2u(u32(uv.x * f32(tex_size.x)), u32(uv.y * f32(tex_size.y)));
    return textureLoad(luminance_mask, tex_coord, 0).r;
}

// Calculate flow direction based on luminance gradient (follow light beams)
fn luminance_flow_dir(uv: vec2f) -> vec2f {
    let eps = 0.02;
    let l = sample_luminance(uv);
    let lx = sample_luminance(uv + vec2f(eps, 0.0));
    let ly = sample_luminance(uv + vec2f(0.0, eps));
    
    // Gradient points from dark to bright (particles flow toward light)
    let grad = vec2f(lx - l, ly - l);
    return normalize(grad + vec2f(0.001)); // Add epsilon to avoid division by zero
}

// Calculate depth based on luminance (brighter = closer/bigger)
fn luminance_depth(lum: f32) -> f32 {
    return lum; // Brighter = closer (0-1 range)
}

// Initialize particle at index
fn init_particle(idx: u32) -> Particle {
    var p: Particle;
    
    // For embers, spawn all at bottom; otherwise random
    if (params.preset == 5u) {
        // Embers: spawn at BOTTOM of screen
        p.position.x = hash(vec2f(f32(idx), 0.0)) * params.width;
        p.position.y = params.height + 8.0; // Start below screen
    } else {
        // Other presets: random spawn
        p.position.x = hash(vec2f(f32(idx), 0.0)) * params.width;
        p.position.y = hash(vec2f(f32(idx), 1.0)) * params.height;
    }
    p.position.z = hash(vec2f(f32(idx), 2.0));
    
    // Calculate initial depth based on local luminance
    let uv = p.position.xy / vec2f(params.width, params.height);
    let local_lum = sample_luminance(uv);
    p.position.z = luminance_depth(local_lum);
    
    p.velocity = vec3f(0.0, 0.0, 0.0);
    p.seed = vec2f(hash(vec2f(f32(idx), 3.0)), hash(vec2f(f32(idx), 4.0)));
    p.size = mix(params.size_min, params.size_max, hash(vec2f(f32(idx), 5.0)));
    p.phase = hash(vec2f(f32(idx), 6.0)) * 6.28;
    p.lifetime = 1.0;
    
    return p;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3u) {
    let idx = global_id.x;
    if (idx >= arrayLength(&particles)) {
        return;
    }
    
    var p = particles[idx];
    
    // Initialize new particles
    if (p.lifetime <= 0.0) {
        p = init_particle(idx);
    }
    
    // Get current luminance context
    let current_uv = p.position.xy / vec2f(params.width, params.height);
    let current_lum = sample_luminance(current_uv);
    
    // Apply preset physics based on preset type
    switch (params.preset) {
        case 0u: { // Cosmic Dust - Full 360° directional flow
            // wind_direction: 0=down, 90=right, 180=up, 270=left (matching config.rs)
            // Convert degrees to radians and compute direction vector
            let drift_dir = params.wind_direction * 6.28318 / 360.0;
            let dir_x = sin(drift_dir);  // 0°→0, 90°→1, 180°→0, 270°→-1
            let dir_y = cos(drift_dir);  // 0°→1, 90°→0, 180°→-1, 270°→0
            
            // Base flow follows wind_direction EXACTLY (no curl interference)
            let base_flow = vec2f(dir_x, dir_y) * params.wind_strength;
            
            // Speed varies per particle for organic feel
            let speed = mix(params.velocity_min, params.velocity_max, p.seed.x);
            
            // DIRECT velocity - no curl noise interference
            p.velocity.x = base_flow.x * speed * 80.0;
            p.velocity.y = base_flow.y * speed * 80.0;
            
            // Move particles
            p.position.x = p.position.x + p.velocity.x * params.delta_time;
            p.position.y = p.position.y + p.velocity.y * params.delta_time;
            
            // Wrap
            if (p.position.x < 0.0) { p.position.x = p.position.x + params.width; }
            if (p.position.x > params.width) { p.position.x = p.position.x - params.width; }
            if (p.position.y < 0.0) { p.position.y = p.position.y + params.height; }
            if (p.position.y > params.height) { p.position.y = p.position.y - params.height; }
            
            // Store seed for color variation in render
            p.position.z = p.seed.y;
            
            // Subtle flicker based on time
            let flicker = sin(params.time * params.flicker_speed + p.phase) * 0.3 + 0.7;
            p.lifetime = (0.3 + current_lum * 0.4) * flicker;
        }
        case 1u: { // Rain - Dynamic wind, gusts, and splash
            // Rain type multipliers (if-else chain instead of array indexing)
            var type_mult: f32;
            if (params.rain_type == 0u) {
                type_mult = 0.3; // drizzle
            } else if (params.rain_type == 1u) {
                type_mult = 1.0; // normal
            } else if (params.rain_type == 2u) {
                type_mult = 1.8; // heavy
            } else {
                type_mult = 2.5; // storm (default)
            }
            
            // Base speed
            let base_speed = mix(params.velocity_min, params.velocity_max, p.seed.x) * 60.0 * type_mult;
            
            // Wind direction in radians
            let wind_rad = params.wind_direction * 3.14159 / 180.0;
            
            // Gust system - sudden wind bursts
            var gust_factor = 0.0;
            if (params.gust_enabled > 0.5) {
                let gust_cycle = params.time * params.gust_frequency * 2.0;
                let gust_wave = sin(gust_cycle) * 0.5 + 0.5;
                let gust_pulse = sin(gust_cycle * 3.14159) * 0.5 + 0.5;
                gust_factor = gust_wave * gust_pulse * params.gust_strength;
            }
            
            // Wind with gusts
            let wind_x = cos(wind_rad) * params.wind_strength * 10.0 + 
                         sin(params.time * 0.5 + p.seed.x * 6.28) * 3.0 * params.turbulence +
                         cos(wind_rad) * gust_factor * 20.0;
            let wind_y = sin(wind_rad) * params.wind_strength * 10.0 + 
                         cos(params.time * 0.3 + p.seed.y * 6.28) * 2.0 * params.turbulence +
                         sin(wind_rad) * gust_factor * 20.0;
            
            // Apply velocities
            p.velocity.x = wind_x;
            p.velocity.y = base_speed + wind_y;
            
            // Stretch effect based on speed
            p.size = mix(params.size_min, params.size_max, p.seed.y) * type_mult * 0.5;
            
            // Move particle
            p.position.x = p.position.x + p.velocity.x * params.delta_time;
            p.position.y = p.position.y + p.velocity.y * params.delta_time;
            
            // Splash/bounce when hitting bottom
            if (params.splash_enabled > 0.5 && p.position.y > params.height) {
                p.velocity.y = -base_speed * params.splash_velocity * 0.3;
                p.position.y = params.height - 5.0;
                p.lifetime = 0.5;
            }
            
            // Wrap around screen
            if (p.position.y > params.height + 50.0) {
                p.position.y = -30.0;
                p.position.x = hash(p.seed + vec2f(params.time, 0.0)) * params.width;
                p.lifetime = 1.0;
            }
            if (p.position.x < 0.0) { p.position.x = p.position.x + params.width; }
            if (p.position.x > params.width) { p.position.x = p.position.x - params.width; }
            
            // Update phase for animation
            p.phase = params.time + p.seed.y * 6.28;
        }
        case 2u: { // Snow - Configurable speed and direction
            // Speed multiplier from config (default 3.0 for compatibility)
            let speed_mult = params.fall_speed_mult;
            
            // Fall direction in degrees (0 = down, 90 = right, 180 = up, 270 = left)
            let fall_dir_rad = params.fall_direction * 3.14159 / 180.0;
            let dir_x = sin(fall_dir_rad);
            let dir_y = cos(fall_dir_rad);
            
            // Base fall speed from velocity_scale
            let base_speed = mix(params.velocity_min, params.velocity_max, p.seed.x) * speed_mult * 60.0;
            
            // Horizontal sine-wave drift
            let drift_phase = p.seed.x * 6.28 + params.time * 0.5;
            let drift_amplitude = params.sway_intensity * 15.0;
            let drift_x = sin(drift_phase) * drift_amplitude;
            
            // Wind influence using turbulence
            let wind_offset = sin(params.time * 0.3 + p.seed.y * 6.28) * params.turbulence * 8.0;
            
            // Combine velocity with fall_direction
            p.velocity.x = (dir_x * base_speed + drift_x + wind_offset) * 0.1;
            p.velocity.y = dir_y * base_speed;
            
            // Apply movement
            p.position.x = p.position.x + p.velocity.x * params.delta_time;
            p.position.y = p.position.y + p.velocity.y * params.delta_time;
            
            // Update phase for continuous animation
            p.phase = p.phase + params.delta_time * 0.5;
            
            // Depth-based bokeh: far particles (small) have lower opacity
            // z = depth, 0 = far, 1 = near
            let depth_factor = 0.2 + p.position.z * 0.8; // Map 0-1 to 0.2-1.0
            p.lifetime = depth_factor; // Far = dimmer, near = brighter
            
            // Reset at bottom - spawn with random drift
            if (p.position.y > params.height) {
                p.position.y = -10.0;
                p.position.x = hash(p.seed + vec2f(params.time, 0.0)) * params.width;
                p.position.z = hash(p.seed + vec2f(params.time, 1.0)); // Random depth
            }
            
            // Soft horizontal wrap
            if (p.position.x < 0.0) { p.position.x = params.width + p.position.x * 0.5; }
            if (p.position.x > params.width) { p.position.x = p.position.x - params.width * 0.5; }
        }
        case 3u: { // Fireflies - FLUYING around with curl noise + bobbing motion
            // Strong curl noise for organic flying pattern
            let curl_scale = 0.003;
            let time_scale = 0.5;
            let curl = curl_noise(p.position.xy * curl_scale, params.time * time_scale);
            
            // Bobbing motion - sinusoidal vertical movement
            let bob_amount = 2.0 + params.turbulence * 3.0;
            let bob_freq = 1.5 + params.flicker_speed * 0.5;
            let bob = sin(params.time * bob_freq + p.phase) * bob_amount;
            
            // Luminance attraction - fly toward bright areas
            let lum = sample_luminance(current_uv);
            let flow_dir = luminance_flow_dir(current_uv);
            let light_attraction = flow_dir * lum * 30.0;
            
            // Strong velocity with curl noise for flying
            let fly_speed = 40.0 + params.velocity_max * 20.0;
            p.velocity.x = p.velocity.x * 0.92 + (curl.x * fly_speed + light_attraction.x) * 0.08;
            p.velocity.y = p.velocity.y * 0.92 + (curl.y * fly_speed + light_attraction.y + bob) * 0.08;
            
            // Move with velocity
            p.position.x = p.position.x + p.velocity.x * params.delta_time;
            p.position.y = p.position.y + p.velocity.y * params.delta_time;
            
            // Soft wrap at boundaries (fly around screen)
            if (p.position.x < -50.0) { p.position.x = params.width + 50.0; }
            if (p.position.x > params.width + 50.0) { p.position.x = -50.0; }
            if (p.position.y < -50.0) { p.position.y = params.height + 50.0; }
            if (p.position.y > params.height + 50.0) { p.position.y = -50.0; }
            
            // Lifetimes based on luminance - brighter areas = longer life
            p.lifetime = 0.5 + lum * 0.5;
        }
        case 4u: { // Sun dust - diverse particles with variety like rain
            let lum = sample_luminance(vec2f(p.position.x, p.position.y) / vec2f(params.width, params.height));
            
            // Diversity based on seed - each particle behaves differently
            let speed_var = p.seed.y; // 0-1 variation
            let is_micro = speed_var < 0.3; // 30% tiny floating dust
            let is_macro = speed_var > 0.7; // 30% larger sun particles
            
            // Size varies a lot for diversity
            if (is_micro) {
                p.size = params.size_min * 0.3; // Very small dust
            } else if (is_macro) {
                p.size = params.size_max * 1.2; // Larger particles
            } else {
                p.size = mix(params.size_min, params.size_max, speed_var);
            }
            
            // Different motion patterns based on particle type (if-else chains)
            var time_scale: f32;
            var turbulence_scale: f32;
            var curl_scale: f32;
            var move_mult: f32;
            
            if (is_micro) {
                time_scale = 0.1;
                turbulence_scale = 0.2;
                curl_scale = 2.0;
                move_mult = 0.2;
            } else if (is_macro) {
                time_scale = 0.4;
                turbulence_scale = 0.8;
                curl_scale = 10.0;
                move_mult = 0.8;
            } else {
                time_scale = 0.2;
                turbulence_scale = 0.5;
                curl_scale = 5.0;
                move_mult = 0.5;
            }
            
            // Curl noise with variation
            let curl = curl_noise(vec2f(p.position.x, p.position.y) * 0.003, params.time * time_scale) * curl_scale;
            
            // Brownian motion varies by particle type
            let brownian = vec2f(
                hash(p.seed + vec2f(params.time * time_scale, 1.0)) - 0.5,
                hash(p.seed + vec2f(params.time * time_scale, 2.0)) - 0.5
            ) * params.turbulence * turbulence_scale;
            
            // Light attraction - macro particles attracted more
            let flow_dir = luminance_flow_dir(vec2f(p.position.x, p.position.y) / vec2f(params.width, params.height));
            let light_attraction = flow_dir * lum * params.turbulence * turbulence_scale * 2.0;
            
            // Macro particles slowly rise (catching light)
            var rise: vec2f;
            if (is_macro) {
                rise = vec2f(0.0, -0.5);
            } else {
                rise = vec2f(0.0, 0.0);
            }
            
            // Combine forces
            let to_light = brownian + light_attraction + curl + rise;
            
            // Smooth velocity with heavy damping for slow flow
            p.velocity.x = p.velocity.x * 0.97 + to_light.x * 0.03;
            p.velocity.y = p.velocity.y * 0.97 + to_light.y * 0.03;
            
            // Movement speed varies by particle type
            p.position.x = p.position.x + p.velocity.x * params.delta_time * move_mult;
            p.position.y = p.position.y + p.velocity.y * params.delta_time * move_mult;
            
            // Alpha varies by size - all very subtle
            if (is_micro) {
                p.lifetime = 0.15;
            } else if (is_macro) {
                p.lifetime = 0.5;
            } else {
                p.lifetime = 0.3;
            }
            
            // Wrap movement
            if (p.position.x < 0.0) { p.position.x = p.position.x + params.width; }
            if (p.position.x > params.width) { p.position.x = p.position.x - params.width; }
            if (p.position.y < 0.0) { p.position.y = params.height; }
            if (p.position.y > params.height) { p.position.y = 0.0; }
        }
        case 5u: { // Embers - concentrated in WARM areas, diverse wind-blown motion
            // Get warm intensity from current position
            let warm_intensity = sample_luminance(current_uv);
            
            // Skip particles in non-warm areas (creates localized clusters)
            if (warm_intensity < 0.15) {
                // In dark areas, ember fades faster
                p.lifetime = p.lifetime - 0.015;
            }
            
            // Random burst angle based on seed (diverse directions, not just straight up)
            let burst_angle = (p.seed.y * 2.0 - 1.0) * 0.6; // -0.6 to 0.6 radians (~30 degrees spread)
            let burst_strength = mix(params.velocity_min, params.velocity_max, p.seed.x);
            let intensity_factor = 0.5 + warm_intensity * 2.0;
            
            // Base burst velocity with random angle
            var burst_x = sin(burst_angle) * burst_strength * 40.0 * intensity_factor;
            var burst_y = -cos(burst_angle) * burst_strength * 50.0 * intensity_factor;
            
            // Apply based on lifetime
            if (p.lifetime > 0.95) {
                // Initial burst - strong
                burst_x = sin(burst_angle) * burst_strength * 70.0 * intensity_factor;
                burst_y = -cos(burst_angle) * burst_strength * 60.0 * intensity_factor;
            } else if (p.lifetime > 0.7) {
                // Decelerating
                burst_x = sin(burst_angle) * burst_strength * 35.0 * intensity_factor;
                burst_y = -cos(burst_angle) * burst_strength * 30.0 * intensity_factor;
            } else if (p.lifetime > 0.3) {
                // Drifting with wind - more horizontal movement
                burst_x = sin(burst_angle) * burst_strength * 15.0 * intensity_factor;
                burst_y = -cos(burst_angle) * burst_strength * 8.0 * intensity_factor;
            } else {
                // Dying - mostly drift
                burst_x = sin(burst_angle) * burst_strength * 5.0;
                burst_y = -cos(burst_angle) * burst_strength * 3.0;
            }
            
            // Dynamic wind - changes over time, different for each particle
            let wind_time = params.time * 0.4 + p.seed.y * 10.0;
            let wind_change = sin(wind_time) * 0.5 + 0.5; // 0-1 oscillation
            let wind_x = sin(wind_time * 0.8 + p.seed.x * 6.28) * params.turbulence * 30.0 * wind_change;
            let wind_y = cos(wind_time * 0.5 + p.seed.y * 6.28) * params.turbulence * 10.0 * wind_change;
            
            // Curl noise for chaotic spark motion
            let curl = curl_noise(p.position.xy * 0.015 + p.seed, params.time * 0.6);
            
            // Combine all forces
            p.velocity.x = p.velocity.x * 0.92 + (burst_x + wind_x) * 0.08;
            p.velocity.y = p.velocity.y * 0.92 + (burst_y + wind_y) * 0.08;
            p.velocity.x = p.velocity.x + curl.x * params.turbulence * 25.0 * params.delta_time;
            p.velocity.y = p.velocity.y + curl.y * params.turbulence * 15.0 * params.delta_time;
            
            // Flicker like burning spark
            let flicker = sin(params.time * params.flicker_speed * 15.0 + p.phase * 3.0) * 0.4 + 0.6;
            p.phase = flicker;
            
            // Move
            p.position.x = p.position.x + p.velocity.x * params.delta_time;
            p.position.y = p.position.y + p.velocity.y * params.delta_time;
            
            // Natural fade out
            p.lifetime = p.lifetime - 0.004;
            
            // Reset - spawn near warm areas (not just bottom)
            if (p.position.y < -30.0 || p.lifetime < 0.05 || warm_intensity < 0.05) {
                // Find a warm spot to spawn
                var spawn_x = params.width * 0.5;
                var spawn_y = params.height * 0.5;
                for (var i = 0u; i < 10u; i = i + 1u) {
                    let test_x = hash(vec2f(f32(idx) + f32(i) * 17.0, 0.0)) * params.width;
                    let test_y = hash(vec2f(f32(idx) + f32(i) * 17.0, 1.0)) * params.height;
                    let test_uv = vec2f(test_x, test_y) / vec2f(params.width, params.height);
                    let test_warm = sample_luminance(test_uv);
                    if (test_warm > 0.3) {
                        spawn_x = test_x;
                        spawn_y = test_y;
                        break;
                    }
                }
                p.position.x = spawn_x;
                p.position.y = spawn_y;
                p.velocity.x = 0.0;
                p.velocity.y = 0.0;
                p.lifetime = 1.0;
            }
            
            // Soft wrap horizontal
            if (p.position.x < 0.0) { p.position.x = params.width - 10.0; }
            if (p.position.x > params.width) { p.position.x = 10.0; }
        }
        default: {
            p.velocity.y = p.velocity.y - 0.1 * params.delta_time;
            p.position.x = p.position.x + p.velocity.x * params.delta_time;
            p.position.y = p.position.y + p.velocity.y * params.delta_time;
        }
    }
    
    particles[idx] = p;
}
"#;

/// Render shader for SDF particle drawing with additive blending
pub const RENDER_SHADER: &str = r#"
// Lumina-RS Render Shader
// SDF-based particle rendering with chameleon effect and depth bokeh

struct Particle {
    position: vec3f,
    velocity: vec3f,
    seed: vec2f,
    lifetime: f32,
    size: f32,
    phase: f32,
}

struct RenderParams {
    time: f32,
    width: f32,
    height: f32,
    preset: u32,
    flicker_speed: f32,
    visibility_ratio: f32,
    padding: vec2f,
    base_color: vec4f,
}

@group(0) @binding(0) var<uniform> params: RenderParams;
@group(0) @binding(1) var<storage, read> particles: array<Particle>;
@group(0) @binding(2) var background: texture_2d<f32>;
@group(0) @binding(3) var luminance_mask: texture_2d<f32>;
@group(0) @binding(4) var tex_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
}

// Full-screen triangle vertex shader
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    var p: vec2f;
    if (vertex_index == 0u) {
        p = vec2f(-1.0, -1.0);
    } else if (vertex_index == 1u) {
        p = vec2f(3.0, -1.0);
    } else {
        p = vec2f(-1.0, 3.0);
    }
    output.position = vec4f(p, 0.0, 1.0);
    // Flip Y for correct texture orientation (origin at top-left)
    output.uv = vec2f(p.x * 0.5 + 0.5, 1.0 - (p.y * 0.5 + 0.5));
    return output;
}

// SDF functions
fn sdf_circle(p: vec2f, center: vec2f, radius: f32) -> f32 {
    return length(p - center) - radius;
}

fn sdf_ellipse(p: vec2f, center: vec2f, radius: vec2f, rotation: f32) -> f32 {
    let c = cos(rotation);
    let s = sin(rotation);
    let rotated = vec2f(
        c * (p.x - center.x) + s * (p.y - center.y),
        -s * (p.x - center.x) + c * (p.y - center.y)
    );
    let normalized = rotated / radius;
    return length(normalized) - 1.0;
}

fn sdf_line(p: vec2f, a: vec2f, b: vec2f, thickness: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - thickness * 0.5;
}

fn bokeh_blur(depth: f32, base_size: f32) -> f32 {
    return base_size * (1.0 + depth * 3.0);
}

// Cosine hue function - converts hue angle (degrees) to RGB color
// Creates smooth rainbow spectrum using cosine palette
fn cos_hue(hue: f32) -> vec3f {
    let h = hue * 3.14159 / 180.0; // Convert to radians
    return vec3f(
        cos(h) * 0.5 + 0.5,
        cos(h + 2.094) * 0.5 + 0.5,  // 120 degrees offset
        cos(h + 4.189) * 0.5 + 0.5   // 240 degrees offset
    );
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    let uv = input.uv;
    let screen_pos = uv * vec2f(params.width, params.height);
    
    let bg_color = textureSample(background, tex_sampler, uv).rgb;
    let lum_mask = textureSample(luminance_mask, tex_sampler, uv).r;
    
    var accum_color = vec3f(0.0);
    var accum_alpha = 0.0;
    
    let particle_count = arrayLength(&particles);
    let is_fireflies = params.preset == 3u;
    
    for (var i = 0u; i < particle_count; i = i + 1u) {
        // For fireflies, use visibility_ratio from config (default: 1 in 10 visible)
        if (is_fireflies && (i % u32(params.visibility_ratio) != 0u)) {
            continue;
        }
        
        let p = particles[i];
        
        if (p.lifetime <= 0.0) {
            continue;
        }
        
        let pos = p.position.xy;
        let depth = p.position.z;
        
        let dist = length(screen_pos - pos);
        let max_dist = p.size * 5.0 + bokeh_blur(depth, 10.0);
        if (dist > max_dist) {
            continue;
        }
        
        let size = bokeh_blur(depth, p.size);
        
        var sdf_dist: f32;
        switch (params.preset) {
            case 0u: {
                // Cosmic Dust - DIVERSE vibrant colors with rainbow spectrum
                let cosmic_size = size * 0.15; // Much smaller
                
                // Get background color at particle position for contextual coloring
                let particle_uv = pos / vec2f(params.width, params.height);
                let bg_at_pos = textureSample(background, tex_sampler, particle_uv).rgb;
                
                // DIVERSE rainbow color spectrum based on seed and time
                // Maps seed to full hue wheel (0-360 degrees)
                let hue1 = p.seed.x * 360.0;                    // Primary hue (0-360)
                let hue2 = p.seed.y * 360.0 + 60.0;              // Secondary hue (shifted)
                let hue3 = (p.seed.x + p.seed.y) * 180.0;       // Tertiary hue
                
                // Convert hues to RGB using cosine palette
                let hue_shift = params.time * 10.0; // Slowly rotate colors over time
                let c1 = cos_hue(hue1 + hue_shift);
                let c2 = cos_hue(hue2 + hue_shift + 120.0);
                let c3 = cos_hue(hue3 + hue_shift + 240.0);
                
                // Create 3-color palette from cosmic spectrum
                let color_a = c1; // Cyan-blue spectrum
                let color_b = c2; // Purple-pink spectrum  
                let color_c = c3; // Gold-orange spectrum
                
                // Mix 3 colors based on position and seed for maximum diversity
                let mix_factor1 = p.seed.x;
                let mix_factor2 = p.seed.y * 0.5 + 0.25;
                let cosmic_color = mix(mix(color_a, color_b, mix_factor1), color_c, mix_factor2);
                
                // Blend with background for contextual feel (subtle, 30%)
                let bg_blend = cosmic_color * 0.7 + bg_at_pos * 0.3;
                
                // Also blend with base_color for preset identity (20%)
                let cosmic_final = mix(bg_blend, params.base_color.rgb, 0.2);
                
                // Tiny bright core + soft glow with rainbow
                let glow_size = cosmic_size * 5.0;
                let glow_sdf = sdf_circle(screen_pos, pos, glow_size);
                let glow_alpha = smoothstep(glow_size, 0.0, glow_sdf) * 0.25;
                let core_alpha = smoothstep(0.2, -0.2, sdf_circle(screen_pos, pos, cosmic_size)) * 0.5;
                
                let total_alpha = glow_alpha + core_alpha;
                
                // Brighter color
                accum_color += cosmic_color * total_alpha * p.lifetime;
                accum_alpha += total_alpha * p.lifetime;
                continue;
            }
            case 1u: {
                // Rain streak: longer, thinner lines based on velocity
                let speed = length(p.velocity.xy);
                let streak_length = size * (1.0 + speed * 0.3); // Longer at higher speed
                let streak_thin = size * 0.15; // Thinner line
                
                // Direction of rain (mostly vertical with slight wind)
                let angle = atan2(p.velocity.y, p.velocity.x + 0.001);
                let top = pos + vec2f(cos(angle), sin(angle)) * streak_length * 0.5;
                let bottom = pos - vec2f(cos(angle), sin(angle)) * streak_length * 0.5;
                sdf_dist = sdf_line(screen_pos, top, bottom, streak_thin);
                
                // Add glow at the bottom (brighter tip)
                let glow = sdf_circle(screen_pos, bottom, size * 0.5);
                sdf_dist = min(sdf_dist, glow);
            }
            case 2u: {
                // Snow: smaller, soft inner glow only (transparent outside)
                let softness = 0.4; // Soft inner core
                let core_size = size * 0.8;
                sdf_dist = sdf_circle(screen_pos, pos, core_size);
                // Calculate alpha here for snow (not in common alpha calc)
                let snow_alpha = smoothstep(softness, -softness, sdf_dist) * p.lifetime;
                if (snow_alpha < 0.1) { continue; }
                // Compute chameleon and lum_factor inline for snow
                let snow_chameleon = mix(params.base_color.rgb, bg_color * 1.5, 0.4);
                let snow_lum_factor = mix(0.3, 1.0, lum_mask);
                // Override common alpha calculation
                accum_color += snow_chameleon * snow_alpha * snow_lum_factor;
                accum_alpha += snow_alpha;
                continue; // Skip common alpha calculation below
            }
            case 3u: {
                // Fireflies: SMALL bright spots using BACKGROUND COLOR + luminance
                // Variable size based on seed - small dots of different sizes
                let size_var = p.seed.x * 2.0; // 0-2x variation
                let base_size = 3.0 + size_var * 4.0; // 3-11 pixels
                
                // Fast sharp flicker - on/off pulse, not smooth fade
                let flicker_t = sin(params.time * params.flicker_speed * 4.0 + p.phase * 6.28) * 0.5 + 0.5;
                let flicker = smoothstep(0.3, 0.7, flicker_t); // Sharp on/off
                
                // Get color from BACKGROUND pixel at particle position
                let particle_uv = pos / vec2f(params.width, params.height);
                let bg_at_pos = textureSample(background, tex_sampler, particle_uv).rgb;
                
                // Luminance-based brightness boost
                // Dark areas = brighter firefly, Bright areas = softer firefly
                let lum_boost = 1.0 + (1.0 - lum_mask) * 1.5; // 1.0-2.5x based on darkness
                
                // Small sharp core + tiny glow
                let core_size = base_size * flicker;
                let glow_size = core_size * 2.0;
                
                let core_sdf = sdf_circle(screen_pos, pos, core_size);
                let glow_sdf = sdf_circle(screen_pos, pos, glow_size);
                
                // Sharp core + subtle glow
                let core_alpha = smoothstep(0.5, -0.5, core_sdf) * flicker;
                let glow_alpha = smoothstep(2.0, 0.0, glow_sdf) * 0.4 * flicker;
                
                // Color from background + luminance boost
                let ff_particle_color = bg_at_pos * lum_boost;
                accum_color += ff_particle_color * (core_alpha + glow_alpha);
                accum_alpha += core_alpha + glow_alpha;
                continue;
            }
            case 4u: { // Sun dust - shimmering particles that don't blur background
                // Sharp core without glow
                sdf_dist = sdf_circle(screen_pos, pos, size);
            }
            case 5u: { // Embers - bright at bottom, fade as rising
                // Small intense spark core
                sdf_dist = sdf_circle(screen_pos, pos, size * 0.8);
            }
            default: {
                sdf_dist = sdf_circle(screen_pos, pos, size);
            }
        }
        
        let alpha = smoothstep(2.0, -2.0, sdf_dist) * p.lifetime;
        let lum_factor = mix(0.3, 1.0, lum_mask);
        let chameleon = mix(params.base_color.rgb, bg_color * 1.5, 0.4);
        
        var particle_color = chameleon;
        switch (params.preset) {
            case 5u: {
                // Intense orange-red glow for embers, more saturated
                let ember_glow = vec3f(1.0, 0.3, 0.05) * (1.0 + p.lifetime * 0.5);
                particle_color = mix(ember_glow, params.base_color.rgb, p.lifetime) * 1.8;
            }
            default: {}
        }
        
        accum_color += particle_color * alpha * lum_factor;
        accum_alpha += alpha;
    }
    
    accum_alpha = min(accum_alpha, 1.0);
    let final_color = bg_color + accum_color * (1.0 - bg_color);
    
    return vec4f(final_color, 1.0);
}
"#;

/// Preprocessed uniform data for GPU
/// Layout must match WGSL std140 alignment (vec4 = 16 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SimParams {
    pub delta_time: f32,
    pub time: f32,
    pub width: f32,
    pub height: f32,
    pub preset: u32,
    pub density: f32,
    pub velocity_min: f32,
    pub velocity_max: f32,
    pub turbulence: f32,
    pub flicker_speed: f32,
    pub size_min: f32,
    pub size_max: f32,
    pub sway_intensity: f32,
    pub buoyancy_force: f32,
    // Snow-specific params
    pub fall_speed_mult: f32,
    pub fall_direction: f32,
    // Fireflies-specific
    pub visibility_ratio: f32,
    pub _pad0: f32,  // padding for vec4 alignment
    pub _pad1: f32,
    pub base_color: [f32; 4],
    // Rain-specific parameters
    pub wind_direction: f32,
    pub wind_strength: f32,
    pub gust_enabled: f32,   // bool as f32
    pub gust_frequency: f32,
    pub gust_strength: f32,
    pub gust_duration: f32,
    pub rain_type: u32,      // 0=drizzle, 1=normal, 2=heavy, 3=storm
    pub splash_enabled: f32, // bool as f32
    pub splash_velocity: f32,
    pub _pad3: f32,  // padding to make size 128 bytes (16-byte alignment)
    pub _pad4: f32,
    pub _pad5: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RenderParams {
    pub time: f32,
    pub width: f32,
    pub height: f32,
    pub preset: u32,
    pub flicker_speed: f32,
    pub visibility_ratio: f32,
    pub padding: [f32; 2],
    pub base_color: [f32; 4],
}

impl Default for SimParams {
    fn default() -> Self {
        Self {
            delta_time: 1.0 / 60.0,
            time: 0.0,
            width: 1920.0,
            height: 1080.0,
            preset: 0,
            density: 1.0,
            velocity_min: 1.0,
            velocity_max: 3.0,
            turbulence: 0.1,
            flicker_speed: 1.0,
            size_min: 2.0,
            size_max: 6.0,
            sway_intensity: 0.5,
            buoyancy_force: 0.0,
            // Snow-specific defaults
            fall_speed_mult: 3.0,
            fall_direction: 0.0,
            // Fireflies-specific defaults
            visibility_ratio: 10.0,
            _pad0: 0.0,
            _pad1: 0.0,
            base_color: [1.0, 1.0, 1.0, 1.0],
            wind_direction: 0.0,
            wind_strength: 0.3,
            gust_enabled: 0.0,
            gust_frequency: 0.15,
            gust_strength: 2.0,
            gust_duration: 0.8,
            rain_type: 1,
            splash_enabled: 0.0,
            splash_velocity: 2.0,
            _pad3: 0.0,
            _pad4: 0.0,
            _pad5: 0.0,
        }
    }
}

impl Default for RenderParams {
    fn default() -> Self {
        Self {
            time: 0.0,
            width: 1920.0,
            height: 1080.0,
            preset: 0,
            flicker_speed: 1.0,
            visibility_ratio: 10.0,
            padding: [0.0, 0.0],
            base_color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}
