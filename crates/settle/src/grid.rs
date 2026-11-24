//! The square planning grid Magna Urbis's `Surface` reads (`urban/surface.rs:15`), cropped from a site raster.
//!
//! Magna Urbis's local frame: `x` east from the crop's west edge, `y` **south** from its north edge (its grid row
//! index grows with `y`, `surface.rs:52-53`), so grid row 0 = north, like the site raster and the sheet.
// Raster, pixel and metre conversions: values are bounded by the raster dims and the sheet size.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
use crate::site::SiteRaster;
use magna_urbis_core::urban::{HeightInterpolation, Point, Surface};
use serde::Serialize;

/// Where Magna Urbis's local frame sits in the world: `world = (west_m + x, north_m - y)`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Frame {
    pub west_m: f64,
    pub north_m: f64,
    /// Side of the square crop (m).
    pub width_m: f64,
    /// Grid side (cells).
    pub n: usize,
}

impl Frame {
    #[must_use]
    pub fn to_world(&self, p: Point) -> [f64; 2] {
        [self.west_m + p.x, self.north_m - p.y]
    }
    #[must_use]
    pub fn to_local(&self, x: f64, y: f64) -> Point {
        Point { x: x - self.west_m, y: self.north_m - y }
    }
}

/// How a site raster becomes the planning grid (values from the recipe).
#[derive(Clone, Copy, Debug)]
pub struct CropSpec {
    /// Half side of the square crop (m).
    pub radius_m: f64,
    /// Planning cell side (m).
    pub spacing_m: f64,
    /// Magna Urbis water code of open water.
    pub water_code: u8,
    /// Vertical scale of the relief about the water level (1 = as given).
    pub relief_scale: f64,
    /// Smaller water bodies plan as dry ground (m²).
    pub min_water_area_m2: f64,
}

pub struct Grid {
    pub frame: Frame,
    pub heights: Vec<f32>,
    /// Magna Urbis water codes (0 = dry).
    pub water: Vec<u8>,
}

impl Grid {
    /// Crop a square of half side `spec.radius_m` around `centre` (world m), shrunk to fit the raster, resampled to
    /// `spec.spacing_m`; heights scaled by `spec.relief_scale` about the site's water level; open water (bodies of
    /// at least `spec.min_water_area_m2`, [`SiteRaster::open_water`]) gets `spec.water_code`.
    ///
    /// # Errors
    /// The centre is outside the raster, or the crop is smaller than 4 cells.
    pub fn crop(site: &SiteRaster, centre: [f64; 2], spec: &CropSpec) -> Result<Self, String> {
        let spacing_m = spec.spacing_m;
        let (west, north, east, south) = site.extent();
        let [cx, cy] = centre;
        let fit = spec.radius_m.min(cx - west).min(east - cx).min(north - cy).min(cy - south);
        if !(fit.is_finite() && fit > 0.0) {
            return Err(format!("centre {centre:?} is outside the site raster"));
        }
        let n = ((2.0 * fit / spacing_m).floor() as usize).max(1);
        if n < 4 {
            return Err(format!("crop of {:.0} m at {spacing_m} m spacing is under 4 cells", 2.0 * fit));
        }
        let width_m = n as f64 * spacing_m;
        let frame = Frame { west_m: cx - width_m / 2.0, north_m: cy + width_m / 2.0, width_m, n };
        let open = site.open_water(spec.min_water_area_m2);
        let level = site.meta.water_level_m;
        let mut heights = Vec::with_capacity(n * n);
        let mut water = Vec::with_capacity(n * n);
        for row in 0..n {
            for col in 0..n {
                let local = Point { x: (col as f64 + 0.5) * spacing_m, y: (row as f64 + 0.5) * spacing_m };
                let [x, y] = frame.to_world(local);
                heights.push((level + (f64::from(site.height_at(x, y)) - level) * spec.relief_scale) as f32);
                let in_water = site.cell_index(x, y).is_some_and(|i| open[i] != 0);
                water.push(if in_water { spec.water_code } else { 0 });
            }
        }
        Ok(Self { frame, heights, water })
    }

    #[must_use]
    pub fn surface(&self) -> Surface<'_> {
        Surface {
            geometry: None,
            width: self.frame.width_m,
            n: self.frame.n,
            height: &self.heights,
            water: &self.water,
            interpolation: HeightInterpolation::Bilinear,
        }
    }
}
