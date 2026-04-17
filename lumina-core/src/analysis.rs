//! Image analysis module - creates luminance mask and context extraction

use image::{DynamicImage, GenericImageView, ImageBuffer, Luma};
use std::path::Path;

/// Luminance mask dimensions
#[derive(Debug, Clone)]
pub struct MaskDimensions {
    pub width: u32,
    pub height: u32,
}

/// Create a luminance mask from an image
/// Value = 0.299R + 0.587G + 0.114B (ITU-R BT.601)
pub fn create_luminance_mask(img: &DynamicImage) -> ImageBuffer<Luma<u8>, Vec<u8>> {
    let (width, height) = img.dimensions();
    
    // Convert to RGB8 if needed
    let rgb = img.to_rgb8();
    
    let mut luminance = ImageBuffer::new(width, height);
    
    for (x, y, pixel) in rgb.enumerate_pixels() {
        // Calculate warm intensity: more weight on red/orange, less on blue
        let warm = (pixel[0] as f32 * 0.5 + pixel[1] as f32 * 0.4 - pixel[2] as f32 * 0.3).max(0.0);
        let lum = (0.299 * pixel[0] as f32 + 
                   0.587 * pixel[1] as f32 + 
                   0.114 * pixel[2] as f32) as u8;
        // Blend luminance with warm color for embers
        let warm_norm = (warm / 255.0 * 0.7 + lum as f32 / 255.0 * 0.3) as u8;
        luminance.put_pixel(x, y, Luma([warm_norm.max(lum)]));
    }
    
    luminance
}

/// Create a warm color mask for embers (highlights red/orange/yellow areas)
pub fn create_warm_mask(img: &DynamicImage) -> ImageBuffer<Luma<u8>, Vec<u8>> {
    let (width, height) = img.dimensions();
    let rgb = img.to_rgb8();
    
    let mut warm = ImageBuffer::new(width, height);
    
    for (x, y, pixel) in rgb.enumerate_pixels() {
        // Warm colors: high red, medium-high green, low blue
        // Formula emphasizes red and orange tones
        let r = pixel[0] as f32;
        let g = pixel[1] as f32;
        let b = pixel[2] as f32;
        
        let warm_val = (r * 0.6 + g * 0.5 - b * 0.4).max(0.0).min(255.0) as u8;
        warm.put_pixel(x, y, Luma([warm_val]));
    }
    
    warm
}

/// Load image from path and create luminance mask
pub fn load_and_analyze<P: AsRef<Path>>(path: P) -> anyhow::Result<(ImageBuffer<Luma<u8>, Vec<u8>>, MaskDimensions)> {
    let img = image::open(path)?;
    let dims = MaskDimensions {
        width: img.width(),
        height: img.height(),
    };
    let mask = create_luminance_mask(&img);
    Ok((mask, dims))
}

/// Get spawn probability at a UV coordinate
/// Returns value between 0.0 and 1.0 based on luminance
pub fn get_spawn_probability(mask: &ImageBuffer<Luma<u8>, Vec<u8>>, u: f32, v: f32) -> f32 {
    let (width, height) = mask.dimensions();
    let x = (u * width as f32) as u32;
    let y = (v * height as f32) as u32;
    let x = x.min(width.saturating_sub(1));
    let y = y.min(height.saturating_sub(1));
    
    mask.get_pixel(x, y)[0] as f32 / 255.0
}

/// Sample background color at UV coordinate (for chameleon effect)
pub fn sample_background_color(img: &DynamicImage, u: f32, v: f32) -> [f32; 4] {
    let (width, height) = img.dimensions();
    let x = (u * width as f32) as u32;
    let y = (v * height as f32) as u32;
    let x = x.min(width.saturating_sub(1));
    let y = y.min(height.saturating_sub(1));
    
    let pixel = img.get_pixel(x, y);
    [
        pixel[0] as f32 / 255.0,
        pixel[1] as f32 / 255.0,
        pixel[2] as f32 / 255.0,
        1.0,
    ]
}

/// Precomputed luminance mask data for GPU upload
pub struct LuminanceMaskData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl LuminanceMaskData {
    /// Create from ImageBuffer
    pub fn from_image_buffer(mask: &ImageBuffer<Luma<u8>, Vec<u8>>) -> Self {
        let (width, height) = mask.dimensions();
        let pixels = mask.as_raw().to_vec();
        Self { width, height, pixels }
    }
    
    /// Total bytes size
    pub fn byte_size(&self) -> usize {
        self.pixels.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_luminance_formula() {
        // Test with actual colored pixel
        let mut img = ImageBuffer::new(1, 1);
        img.put_pixel(0, 0, image::Rgb([255, 0, 0])); // Pure red
        let dyn_img = DynamicImage::ImageRgb8(img);
        let mask = create_luminance_mask(&dyn_img);
        // 0.299 * 255 ≈ 76
        assert!((mask.get_pixel(0, 0)[0] as f32 - 76.0).abs() < 1.0);
    }
}
