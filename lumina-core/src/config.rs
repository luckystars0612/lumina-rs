//! Configuration types for Lumina-RS overlay rendering

use serde::{Deserialize, Serialize};

/// Render configuration for output video
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderConfig {
    /// Output width in pixels
    pub width: u32,
    /// Output height in pixels
    pub height: u32,
    /// Frames per second
    pub fps: u32,
    /// Duration in seconds (for seamless loop)
    pub duration_secs: u32,
    /// Particle count (100-5000 range)
    pub particle_count: u32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 60,
            duration_secs: 10,
            particle_count: 5000,
        }
    }
}

impl RenderConfig {
    /// Validate and clamp particle count to valid range
    pub fn validate(&mut self) {
        self.particle_count = self.particle_count.clamp(100, 5000);
        
        // Ensure reasonable dimensions
        self.width = self.width.clamp(360, 3840);
        self.height = self.height.clamp(360, 2160);
        self.fps = self.fps.clamp(24, 120);
        self.duration_secs = self.duration_secs.clamp(1, 60);
    }
}

/// Lumina-RS particle presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    CosmicDust = 0,
    Rain = 1,
    Snow = 2,
    Fireflies = 3,
    SunDust = 4,
    Embers = 5,
}

impl Preset {
    /// Convert preset to shader index (u32)
    pub fn to_shader_index(&self) -> u32 {
        *self as u32
    }
    
    /// Parse from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cosmic_dust" | "cosmicdust" | "0" => Some(Preset::CosmicDust),
            "rain" | "1" => Some(Preset::Rain),
            "snow" | "2" => Some(Preset::Snow),
            "fireflies" | "fire_fly" | "3" => Some(Preset::Fireflies),
            "sun_dust" | "sundust" | "4" => Some(Preset::SunDust),
            "embers" | "5" => Some(Preset::Embers),
            _ => None,
        }
    }
}

/// Rain type variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RainType {
    Drizzle = 0,
    Normal = 1,
    Heavy = 2,
    Storm = 3,
}

impl RainType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "drizzle" | "0" => Some(RainType::Drizzle),
            "normal" | "1" => Some(RainType::Normal),
            "heavy" | "2" => Some(RainType::Heavy),
            "storm" | "3" => Some(RainType::Storm),
            _ => None,
        }
    }
}

/// Common overlay parameters (all presets support these)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OverlayParams {
    #[serde(default = "default_density")]
    pub density_multiplier: f32,
    #[serde(default = "default_velocity")]
    pub velocity_scale: [f32; 2],
    #[serde(default = "default_color")]
    pub base_color_hex: String,
    #[serde(default = "default_turbulence")]
    pub turbulence: f32,
    #[serde(default = "default_flicker")]
    pub flicker_speed: f32,
    #[serde(default = "default_size")]
    pub size_range: [f32; 2],
    // Rain-specific parameters
    #[serde(default)]
    pub rain_type: Option<RainType>,
    #[serde(default)]
    pub wind_direction: f32,
    #[serde(default)]
    pub wind_strength: f32,
    #[serde(default)]
    pub gust_enabled: bool,
    #[serde(default)]
    pub gust_frequency: f32,
    #[serde(default)]
    pub gust_strength: f32,
    #[serde(default)]
    pub gust_duration: f32,
    #[serde(default)]
    pub splash_enabled: bool,
    #[serde(default)]
    pub splash_velocity: f32,
    // Snow-specific
    #[serde(default)]
    pub sway_intensity: f32,
    #[serde(default)]
    pub fall_speed_mult: f32,
    #[serde(default)]
    pub fall_direction: f32,
    // Fireflies-specific
    #[serde(default)]
    pub visibility_ratio: u32,
}

// Default values
fn default_density() -> f32 { 1.0 }
fn default_velocity() -> [f32; 2] { [1.0, 3.0] }
fn default_color() -> String { "#FFFFFF".to_string() }
fn default_turbulence() -> f32 { 0.1 }
fn default_flicker() -> f32 { 1.0 }
fn default_size() -> [f32; 2] { [2.0, 6.0] }

/// Full overlay configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayConfig {
    pub preset: Preset,
    #[serde(default)]
    pub params: OverlayParams,
    /// Optional render configuration (defaults to 1920x1080 @ 60fps, 5000 particles)
    #[serde(default)]
    pub render: Option<RenderConfig>,
}

/// Wrapper for nested intent format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentWrapper {
    pub overlay_config: OverlayConfig,
}

/// Convert hex color to RGBA array for shaders
pub fn hex_to_rgba(hex: &str) -> [f32; 4] {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255) as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255) as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255) as f32 / 255.0;
    let a = if hex.len() >= 8 {
        u8::from_str_radix(&hex[6..8], 16).unwrap_or(255) as f32 / 255.0
    } else {
        1.0
    };
    [r, g, b, a]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_rgba() {
        let rgba = hex_to_rgba("#FF0000");
        assert!((rgba[0] - 1.0).abs() < 0.01);
        assert!((rgba[1] - 0.0).abs() < 0.01);
        assert!((rgba[2] - 0.0).abs() < 0.01);
        assert!((rgba[3] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_preset_from_str() {
        assert_eq!(Preset::from_str("rain"), Some(Preset::Rain));
        assert_eq!(Preset::from_str("RAIN"), Some(Preset::Rain));
        assert_eq!(Preset::from_str("snow"), Some(Preset::Snow));
        assert_eq!(Preset::from_str("invalid"), None);
    }

    #[test]
    fn test_render_config_validate() {
        let mut config = RenderConfig {
            width: 1920,
            height: 1080,
            fps: 60,
            duration_secs: 10,
            particle_count: 10000, // Over max
        };
        config.validate();
        assert_eq!(config.particle_count, 5000);
        
        config.particle_count = 50; // Under min
        config.validate();
        assert_eq!(config.particle_count, 100);
    }
}
