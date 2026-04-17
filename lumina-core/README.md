# Lumina-RS Preset Documentation

## Overview

Lumina-RS provides **6 particle presets** with distinct physics and rendering. All presets interact with the luminance mask derived from the background image.

---

## Preset Index Mapping

| Index | Preset Name | File |
|-------|-------------|------|
| 0 | CosmicDust | intent6.json |
| 1 | Rain | intent1.json |
| 2 | Snow | intent4.json |
| 3 | Fireflies | - |
| 4 | SunDust | intent2.json |
| 5 | Embers | intent3.json |

---

## Common Parameters

All presets support these shared parameters:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `density_multiplier` | f32 | 1.0 | Particle count multiplier (0.1 - 2.0) |
| `velocity_scale` | [f32; 2] | [1.0, 3.0] | [min, max] velocity range |
| `base_color_hex` | String | "#FFFFFF" | Primary color in HEX format (#RRGGBB) |
| `turbulence` | f32 | 0.1 | Turbulence/chaos intensity (0.0 - 1.0) |
| `flicker_speed` | f32 | 1.0 | Flicker animation speed |
| `size_range` | [f32; 2] | [2.0, 6.0] | [min, max] particle size in pixels |
| `sway_intensity` | f32 | 0.5 | Horizontal sway amplitude (Snow/Embers) |

### Color Format
Use HEX format: `#RRGGBB`
- `#FF0000` = Red
- `#00FF00` = Green
- `#0000FF` = Blue
- `#FFFF00` = Yellow
- `#FF00FF` = Magenta
- `#00FFFF` = Cyan

---

## 1. Cosmic Dust (Index 0)

### Description
Tiny particles with vibrant rainbow spectrum colors, drifting in customizable wind directions. Ideal for space dust, shimmering light, or sparkle effects.

### Physics
- **Direction**: Full 360° flow based on `wind_direction`
- **Speed**: Linear with per-particle variation
- **Flicker**: Subtle pulsing over time

### Rendering
- **Size**: Very small (15% of base size)
- **Color**: Rainbow spectrum with 3 main colors (cyan-blue, purple-pink, gold-orange)
- **Hue Shift**: Colors slowly rotate over time
- **Blend**: 30% background color, 20% base_color

### Specific Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `wind_direction` | f32 | 0.0 | Wind direction (0=down, 90=right, 180=up, 270=left) |
| `wind_strength` | f32 | 0.3 | Wind intensity (0.0 - 2.0) |

### Example JSON
```json
{
    "overlay_config": {
        "preset": "cosmic_dust",
        "params": {
            "density_multiplier": 2.5,
            "velocity_scale": [1.0, 2.0],
            "base_color_hex": "#9b59ff",
            "turbulence": 0.5,
            "flicker_speed": 0.3,
            "size_range": [0.5, 1.5],
            "wind_direction": 180,
            "wind_strength": 1.0
        }
    }
}
```

---

## 2. Rain (Index 1)

### Description
Long streak particles falling vertically with wind effects, gusts (sudden wind bursts), and optional splash when hitting the bottom. Ideal for rain, storms, or similar effects.

### Physics
- **Fall Speed**: Based on `rain_type` and velocity_scale
- **Wind**: Affects fall angle based on `wind_direction`
- **Gusts**: Sudden wind bursts with configurable frequency and strength
- **Splash**: Light bounce when hitting bottom (optional)

### Rain Types

| Type | Index | Description | Speed Multiplier |
|------|-------|-------------|-----------------|
| drizzle | 0 | Light rain, slow fall | 0.3x |
| normal | 1 | Moderate rain | 1.0x |
| heavy | 2 | Heavy rain, fast fall | 1.8x |
| storm | 3 | Storm with extreme speed | 2.5x |

### Rendering
- **Shape**: Long streak instead of circle
- **Stretch**: Longer at higher speeds
- **Glow**: Bright tip at the tail

### Specific Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `wind_direction` | f32 | 0.0 | Fall angle direction (degrees) |
| `wind_strength` | f32 | 0.3 | Wind intensity (0.0 - 2.0) |
| `gust_enabled` | bool | false | Enable sudden wind gusts |
| `gust_frequency` | f32 | 0.15 | Gust frequency (0.0 - 1.0) |
| `gust_strength` | f32 | 2.0 | Gust intensity (1.0 - 3.0) |
| `gust_duration` | f32 | 0.8 | Duration of each gust (seconds) |
| `rain_type` | String | "normal" | Type: drizzle/normal/heavy/storm |
| `splash_enabled` | bool | false | Enable splash on bottom hit |
| `splash_velocity` | f32 | 2.0 | Splash bounce speed |

### Example JSON
```json
{
    "overlay_config": {
        "preset": "rain",
        "params": {
            "density_multiplier": 0.2,
            "velocity_scale": [8.0, 12.0],
            "base_color_hex": "#aaccff",
            "turbulence": 0.3,
            "flicker_speed": 0.0,
            "size_range": [1.0, 2.0],
            "rain_type": "normal",
            "wind_direction": 35.0,
            "wind_strength": 1.8,
            "gust_enabled": true,
            "gust_frequency": 0.15,
            "gust_strength": 3.0,
            "gust_duration": 1.2,
            "splash_enabled": false,
            "splash_velocity": 2.0
        }
    }
}
```

---

## 3. Snow (Index 2)

### Description
Soft particles falling slowly with horizontal drift and depth-based bokeh. Ideal for snowfall, feathers, or gentle particle effects.

### Physics
- **Fall Speed**: Slow (3x velocity scale)
- **Horizontal Drift**: Sine-wave sway
- **Wind Influence**: Turbulence affects horizontal drift
- **Depth**: z-position affects opacity (far = dimmer)

### Rendering
- **Shape**: Soft circle with smooth edge
- **Bokeh**: Depth-based blur (near = larger, brighter)
- **Color**: Chameleon effect - blends with background
- **Softness**: Hardcoded 0.4 (soft inner core)

### Specific Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `sway_intensity` | f32 | 0.5 | Horizontal sway amplitude |

**Note**: `opacity` and `softness` parameters in JSON are currently not implemented (uses hardcoded values).

### Example JSON
```json
{
    "overlay_config": {
        "preset": "snow",
        "params": {
            "density_multiplier": 1.5,
            "velocity_scale": [0.5, 1.2],
            "base_color_hex": "#e8f4ff",
            "turbulence": 0.3,
            "flicker_speed": 0.0,
            "size_range": [0.5, 1.5],
            "sway_intensity": 1.2
        }
    }
}
```

---

## 4. Fireflies (Index 3)

### Description
Small particles freely flying with curl noise and bobbing motion, attracted toward light sources. Ideal for fireflies, sparkles, or magical light effects.

### Physics
- **Motion**: Organic flying pattern with curl noise
- **Bobbing**: Vertical movement in sine wave
- **Light Attraction**: Flies toward bright areas (luminance gradient)
- **Visibility**: Only 1% of particles visible at any time (~50 visible)

### Rendering
- **Size**: Small, variable (3-11 pixels)
- **Flicker**: Sharp on/off pulse (not smooth fade)
- **Color**: From background pixel + luminance boost
- **Dark Areas**: Fireflies brighter in dark regions

### Specific Parameters
Uses only common parameters.

### Example JSON
```json
{
    "overlay_config": {
        "preset": "fireflies",
        "params": {
            "density_multiplier": 3.0,
            "velocity_scale": [1.0, 2.0],
            "base_color_hex": "#ffd700",
            "turbulence": 0.3,
            "flicker_speed": 2.0,
            "size_range": [2.0, 5.0]
        }
    }
}
```

---

## 5. Sun Dust (Index 4)

### Description
Diverse particles with 3 different sizes (micro, medium, macro) floating gently in air. Ideal for sunlight, dust motes, or ethereal effects.

### Physics
- **Micro Dust** (30%): Very small, slow movement, low alpha
- **Medium Dust** (40%): Medium size, moderate movement
- **Macro Dust** (30%): Larger, faster movement, has rise effect

### Motion Patterns

| Type | Size | Speed | Alpha | Special |
|------|------|-------|-------|---------|
| Micro | 0.3x min | 0.2x | 0.15 | Slow float |
| Medium | normal | 0.5x | 0.3 | Brownian |
| Macro | 1.2x max | 0.8x | 0.5 | Rising + light attraction |

### Rendering
- **Shape**: Sharp core without glow
- **Background**: Does not blend with background

### Specific Parameters
Uses only common parameters.

### Example JSON
```json
{
    "overlay_config": {
        "preset": "sun_dust",
        "params": {
            "density_multiplier": 0.003,
            "velocity_scale": [0.1, 0.3],
            "base_color_hex": "#ffeedd",
            "turbulence": 0.15,
            "flicker_speed": 0.5,
            "size_range": [3.0, 5.0]
        }
    }
}
```

---

## 6. Embers (Index 5)

### Description
Particles concentrated in bright areas, rising with initial burst and natural fade out. Ideal for fire, sparks, or campfire embers.

### Physics
- **Spawn**: Only spawns in areas with luminance > 0.3
- **Burst**: Strong initial velocity, decreasing with lifetime
- **Wind**: Dynamic wind changing over time
- **Curl Noise**: Chaotic movement
- **Fade**: Natural fade out based on lifetime

### Velocity Phases

| Lifetime | Burst X | Burst Y | Behavior |
|----------|---------|---------|----------|
| > 95% | 70x | 60x | Strong initial burst |
| 70-95% | 35x | 30x | Decelerating |
| 30-70% | 15x | 8x | Drifting with wind |
| < 30% | 5x | 3x | Dying - mostly drift |

### Rendering
- **Shape**: Small intense spark core
- **Color**: Orange-red glow with high saturation
- **Lifetime**: Brighter when spawned, fades when dying

### Specific Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `sway_intensity` | f32 | 0.5 | Drift intensity |

### Example JSON
```json
{
    "overlay_config": {
        "preset": "embers",
        "params": {
            "density_multiplier": 1.0,
            "velocity_scale": [3.0, 6.0],
            "base_color_hex": "#ff4400",
            "turbulence": 0.6,
            "flicker_speed": 3.0,
            "size_range": [1.0, 2.5],
            "sway_intensity": 0.8
        }
    }
}
```

---

## Usage

### Run Lumina-RS
```bash
cargo run --release -- \
    --input resources/background.png \
    --intent resources/intent/intent1.json \
    --output output.mp4
```

### Parameters File Location
Place intent files in `resources/intent/`:
```
lumina-core/
├── resources/
│   └── intent/
│       ├── intent1.json  (Rain)
│       ├── intent2.json  (SunDust)
│       ├── intent3.json  (Embers)
│       ├── intent4.json  (Snow)
│       └── intent6.json  (CosmicDust)
```

### Build Commands
```bash
# Check code
cargo check

# Build release
cargo build --release
```

---

## Tips & Tricks

### Cosmic Dust
- `wind_direction: 180` + `wind_strength: 1.0` = Particles rise upward
- Decrease `size_range` for smaller particles
- Increase `density_multiplier` for denser effect

### Rain
- `rain_type: "storm"` + `gust_enabled: true` for convincing storm effect
- Adjust `wind_direction` to change fall angle
- `splash_enabled: true` for bounce effect

### Snow
- Lower `velocity_scale` for slow, romantic snowfall
- Increase `sway_intensity` for more horizontal drift
- Use light gray (`#e8f4f8`) for natural snow

### Fireflies
- Increase `density_multiplier` since only 1% is visible
- Higher `flicker_speed` for lively animation
- Test on dark backgrounds for best visibility

### Sun Dust
- Bright backgrounds make particles stand out
- Low `turbulence` for smooth motion
- Great for sunlight/beam effects

### Embers
- Works best with bright/warm areas in background
- Increase `turbulence` for more chaotic motion
- Higher `flicker_speed` for realistic burning effect

---

## Unsupported Parameters

The following parameters exist in some intent JSONs but are NOT currently implemented:
- `opacity` (Snow) - uses hardcoded internal value
- `softness` (Snow) - uses hardcoded 0.4
- `secondary_color_hex` (CosmicDust) - not used
- `interaction_logic` / `director_note` - comments only, not parsed
