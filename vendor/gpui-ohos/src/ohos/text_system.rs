use anyhow::{Context as _, Result};
use cosmic_text::{
    Attrs, AttrsList, CacheKey, Family, Font as CosmicTextFont, FontFeatures as CosmicFontFeatures,
    FontSystem, ShapeBuffer, ShapeLine, SwashCache,
};
use gpui::{
    Bounds, DevicePixels, Font, FontFeatures, FontId, FontMetrics, FontRun, FontStyle, FontWeight,
    GlyphId, LineLayout, Pixels, PlatformTextSystem, RenderGlyphParams, SUBPIXEL_VARIANTS_X,
    SUBPIXEL_VARIANTS_Y, ShapedGlyph, ShapedRun, SharedString, Size, TextRenderingMode,
    font_name_with_fallbacks, point, size,
};
use itertools::Itertools;
use log::warn;
use parking_lot::RwLock;
use rustc_hash::FxHashMap as HashMap;
use smallvec::SmallVec;
use std::{borrow::Cow, path::Path, sync::Arc};

pub(crate) struct OhosTextSystem(RwLock<OhosTextSystemState>);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FontKey {
    family: SharedString,
    features: FontFeatures,
    weight: FontWeight,
    style: FontStyle,
}

impl FontKey {
    fn new(
        family: SharedString,
        features: FontFeatures,
        weight: FontWeight,
        style: FontStyle,
    ) -> Self {
        Self {
            family,
            features,
            weight,
            style,
        }
    }
}

struct OhosTextSystemState {
    swash_cache: SwashCache,
    font_system: FontSystem,
    scratch: ShapeBuffer,
    loaded_fonts: Vec<LoadedFont>,
    font_ids_by_family_cache: HashMap<FontKey, SmallVec<[FontId; 4]>>,
    system_fonts_loaded: bool,
}

struct LoadedFont {
    font: Arc<CosmicTextFont>,
    font_weight: cosmic_text::Weight,
    features: CosmicFontFeatures,
    is_known_emoji_font: bool,
}

impl OhosTextSystem {
    pub(crate) fn new() -> Self {
        let font_system = FontSystem::new();

        Self(RwLock::new(OhosTextSystemState {
            font_system,
            swash_cache: SwashCache::new(),
            scratch: ShapeBuffer::default(),
            loaded_fonts: Vec::new(),
            font_ids_by_family_cache: HashMap::default(),
            system_fonts_loaded: false,
        }))
    }
}

impl Default for OhosTextSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformTextSystem for OhosTextSystem {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        self.0.write().add_fonts(fonts)
    }

    fn all_font_names(&self) -> Vec<String> {
        let mut result = self
            .0
            .read()
            .font_system
            .db()
            .faces()
            .filter_map(|face| face.families.first().map(|family| family.0.clone()))
            .collect_vec();
        result.sort();
        result.dedup();
        result
    }

    fn font_id(&self, font: &Font) -> Result<FontId> {
        let mut state = self.0.write();
        let key = FontKey::new(
            font.family.clone(),
            font.features.clone(),
            font.weight,
            font.style,
        );
        let candidates = if let Some(font_ids) = state.font_ids_by_family_cache.get(&key) {
            font_ids.as_slice()
        } else {
            let font_ids =
                state.load_family(&font.family, &font.features, font.weight, font.style)?;
            state.font_ids_by_family_cache.insert(key.clone(), font_ids);
            state.font_ids_by_family_cache[&key].as_ref()
        };
        anyhow::ensure!(
            !candidates.is_empty(),
            "no candidate fonts for family '{}'",
            font.family
        );

        let candidate_properties = candidates
            .iter()
            .filter_map(|font_id| {
                let database_id = state.loaded_font(*font_id).font.id();
                state
                    .font_system
                    .db()
                    .face(database_id)
                    .map(face_info_into_properties)
            })
            .collect::<SmallVec<[_; 4]>>();

        let index = if candidate_properties.is_empty() {
            0
        } else {
            match font_kit::matching::find_best_match(
                &candidate_properties,
                &font_into_properties(font),
            ) {
                Ok(index) => index,
                Err(_) => 0,
            }
        };

        Ok(candidates[index])
    }

    fn font_metrics(&self, font_id: FontId) -> FontMetrics {
        let metrics = self
            .0
            .read()
            .loaded_font(font_id)
            .font
            .as_swash()
            .metrics(&[]);

        FontMetrics {
            units_per_em: metrics.units_per_em as u32,
            ascent: metrics.ascent,
            descent: -metrics.descent,
            line_gap: metrics.leading,
            underline_position: metrics.underline_offset,
            underline_thickness: metrics.stroke_size,
            cap_height: metrics.cap_height,
            x_height: metrics.x_height,
            bounding_box: Bounds {
                origin: point(0.0, 0.0),
                size: size(metrics.max_width, metrics.ascent + metrics.descent),
            },
        }
    }

    fn typographic_bounds(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Bounds<f32>> {
        let lock = self.0.read();
        let glyph_metrics = lock.loaded_font(font_id).font.as_swash().glyph_metrics(&[]);
        let glyph_id = glyph_id.0 as u16;
        Ok(Bounds {
            origin: point(0.0, 0.0),
            size: size(
                glyph_metrics.advance_width(glyph_id),
                glyph_metrics.advance_height(glyph_id),
            ),
        })
    }

    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>> {
        self.0.read().advance(font_id, glyph_id)
    }

    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId> {
        self.0.read().glyph_for_char(font_id, ch)
    }

    fn glyph_raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        self.0.write().raster_bounds(params)
    }

    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        self.0.write().rasterize_glyph(params, raster_bounds)
    }

    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        self.0.write().layout_line(text, font_size, runs)
    }

    fn recommended_rendering_mode(
        &self,
        _font_id: FontId,
        _font_size: Pixels,
    ) -> TextRenderingMode {
        TextRenderingMode::Grayscale
    }
}

impl OhosTextSystemState {
    fn normalize_family_name(name: &str) -> String {
        name.trim()
            .to_lowercase()
            .replace('_', " ")
            .replace('-', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn ensure_system_fonts_loaded(&mut self) {
        if self.system_fonts_loaded {
            return;
        }

        let db = self.font_system.db_mut();
        let initial_count = db.faces().count();

        const FONT_DIRS: [&str; 5] = [
            "/system/fonts",
            "/system/font",
            "/vendor/fonts",
            "/system_ext/fonts",
            "/data/fonts",
        ];
        for dir in FONT_DIRS {
            let path = Path::new(dir);
            if path.is_dir() {
                db.load_fonts_dir(path);
            }
        }

        let final_count = db.faces().count();
        if final_count == initial_count {
            db.load_system_fonts();
        }

        self.system_fonts_loaded = true;
        if db.faces().next().is_none() {
            warn!(
                "OHOS text system: no system fonts found in common directories or fontdb defaults"
            );
        }
    }

    fn loaded_font(&self, font_id: FontId) -> &LoadedFont {
        &self.loaded_fonts[font_id.0]
    }

    fn add_fonts(&mut self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        let db = self.font_system.db_mut();
        for bytes in fonts {
            match bytes {
                Cow::Borrowed(embedded_font) => {
                    db.load_font_data(embedded_font.to_vec());
                }
                Cow::Owned(bytes) => {
                    db.load_font_data(bytes);
                }
            }
        }
        Ok(())
    }

    fn load_family(
        &mut self,
        name: &str,
        features: &FontFeatures,
        _requested_weight: FontWeight,
        _requested_style: FontStyle,
    ) -> Result<SmallVec<[FontId; 4]>> {
        self.ensure_system_fonts_loaded();

        let mut families = SmallVec::<[(cosmic_text::fontdb::ID, String); 4]>::new();
        let mut seen_ids = HashMap::<cosmic_text::fontdb::ID, ()>::default();

        let system_name = "HarmonyOS Sans";
        let primary_name = font_name_with_fallbacks(name, system_name);
        let mut candidates = SmallVec::<[&str; 5]>::new();
        candidates.push(primary_name);
        candidates.push("HarmonyOS Sans");
        candidates.push("HarmonyOS_Sans");
        candidates.push("sans-serif");
        candidates.push("Noto Sans");

        for candidate in candidates {
            let normalized_candidate = Self::normalize_family_name(candidate);
            for face in self.font_system.db().faces().filter(|face| {
                face.families.iter().any(|family| {
                    candidate == family.0
                        || normalized_candidate == Self::normalize_family_name(&family.0)
                })
            }) {
                if seen_ids.insert(face.id, ()).is_none() {
                    families.push((face.id, face.post_script_name.clone()));
                }
            }
            if !families.is_empty() {
                break;
            }
        }

        if families.is_empty() {
            let normalized_system_name = Self::normalize_family_name(system_name);
            for face in self.font_system.db().faces().filter(|face| {
                face.families.iter().any(|family| {
                    system_name == family.0
                        || normalized_system_name == Self::normalize_family_name(&family.0)
                })
            }) {
                if seen_ids.insert(face.id, ()).is_none() {
                    families.push((face.id, face.post_script_name.clone()));
                }
            }
        }

        if families.is_empty() {
            if let Some(first_face) = self.font_system.db().faces().next() {
                warn!(
                    "OHOS text system: no family match for '{name}', fallback to first font '{}'",
                    first_face.post_script_name
                );
                families.push((first_face.id, first_face.post_script_name.clone()));
            } else {
                anyhow::bail!("OHOS text system: no system fonts available");
            }
        }

        let mut loaded_font_ids = SmallVec::new();
        for (font_id, postscript_name) in families {
            let font_weight = self
                .font_system
                .db()
                .face(font_id)
                .map(|face| face.weight)
                .unwrap_or(cosmic_text::Weight::NORMAL);
            let font = self
                .font_system
                .get_font(font_id, font_weight)
                .context("could not load font")?;

            let font_id = FontId(self.loaded_fonts.len());
            loaded_font_ids.push(font_id);
            self.loaded_fonts.push(LoadedFont {
                font,
                font_weight,
                features: cosmic_font_features_from(features)?,
                is_known_emoji_font: check_is_known_emoji_font(&postscript_name),
            });
        }

        Ok(loaded_font_ids)
    }

    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>> {
        let glyph_metrics = self.loaded_font(font_id).font.as_swash().glyph_metrics(&[]);
        Ok(Size {
            width: glyph_metrics.advance_width(glyph_id.0 as u16),
            height: glyph_metrics.advance_height(glyph_id.0 as u16),
        })
    }

    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId> {
        let glyph_id = self.loaded_font(font_id).font.as_swash().charmap().map(ch);
        if glyph_id == 0 {
            None
        } else {
            Some(GlyphId(glyph_id.into()))
        }
    }

    fn raster_bounds(&mut self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        let loaded_font = &self.loaded_fonts[params.font_id.0];
        let font = &loaded_font.font;
        let subpixel_shift = point(
            params.subpixel_variant.x as f32 / SUBPIXEL_VARIANTS_X as f32 / params.scale_factor,
            params.subpixel_variant.y as f32 / SUBPIXEL_VARIANTS_Y as f32 / params.scale_factor,
        );
        let image = self
            .swash_cache
            .get_image(
                &mut self.font_system,
                CacheKey::new(
                    font.id(),
                    params.glyph_id.0 as u16,
                    (params.font_size * params.scale_factor).into(),
                    (subpixel_shift.x, subpixel_shift.y.trunc()),
                    loaded_font.font_weight,
                    cosmic_text::CacheKeyFlags::empty(),
                )
                .0,
            )
            .clone()
            .with_context(|| format!("no image for {params:?} in font {font:?}"))?;
        Ok(Bounds {
            origin: point(image.placement.left.into(), (-image.placement.top).into()),
            size: size(image.placement.width.into(), image.placement.height.into()),
        })
    }

    fn rasterize_glyph(
        &mut self,
        params: &RenderGlyphParams,
        glyph_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        if glyph_bounds.size.width.0 == 0 || glyph_bounds.size.height.0 == 0 {
            anyhow::bail!("glyph bounds are empty");
        }

        let bitmap_size = glyph_bounds.size;
        let loaded_font = &self.loaded_fonts[params.font_id.0];
        let font = &loaded_font.font;
        let subpixel_shift = point(
            params.subpixel_variant.x as f32 / SUBPIXEL_VARIANTS_X as f32 / params.scale_factor,
            params.subpixel_variant.y as f32 / SUBPIXEL_VARIANTS_Y as f32 / params.scale_factor,
        );
        let mut image = self
            .swash_cache
            .get_image(
                &mut self.font_system,
                CacheKey::new(
                    font.id(),
                    params.glyph_id.0 as u16,
                    (params.font_size * params.scale_factor).into(),
                    (subpixel_shift.x, subpixel_shift.y.trunc()),
                    loaded_font.font_weight,
                    cosmic_text::CacheKeyFlags::empty(),
                )
                .0,
            )
            .clone()
            .with_context(|| format!("no image for {params:?} in font {font:?}"))?;

        if params.is_emoji {
            for pixel in image.data.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
        }

        Ok((bitmap_size, image.data))
    }

    fn font_id_for_cosmic_id(&mut self, id: cosmic_text::fontdb::ID) -> FontId {
        if let Some(index) = self
            .loaded_fonts
            .iter()
            .position(|loaded_font| loaded_font.font.id() == id)
        {
            FontId(index)
        } else {
            let font = self
                .font_system
                .get_font(
                    id,
                    self.font_system
                        .db()
                        .face(id)
                        .map(|face| face.weight)
                        .unwrap_or(cosmic_text::Weight::NORMAL),
                )
                .expect("font id should be valid");
            let face = self
                .font_system
                .db()
                .face(id)
                .expect("font face should exist");

            let font_id = FontId(self.loaded_fonts.len());
            self.loaded_fonts.push(LoadedFont {
                font,
                font_weight: face.weight,
                features: CosmicFontFeatures::new(),
                is_known_emoji_font: check_is_known_emoji_font(&face.post_script_name),
            });

            font_id
        }
    }

    fn layout_line(&mut self, text: &str, font_size: Pixels, font_runs: &[FontRun]) -> LineLayout {
        let mut attrs_list = AttrsList::new(&Attrs::new());
        let mut offset = 0;
        for run in font_runs {
            let loaded_font = self.loaded_font(run.font_id);
            let font = self
                .font_system
                .db()
                .face(loaded_font.font.id())
                .expect("font face should exist");

            attrs_list.add_span(
                offset..(offset + run.len),
                &Attrs::new()
                    .metadata(run.font_id.0)
                    .family(Family::Name(
                        &font.families.first().expect("font family should exist").0,
                    ))
                    .stretch(font.stretch)
                    .style(font.style)
                    .weight(font.weight)
                    .font_features(loaded_font.features.clone()),
            );
            offset += run.len;
        }

        let line = ShapeLine::new(
            &mut self.font_system,
            text,
            &attrs_list,
            cosmic_text::Shaping::Advanced,
            4,
        );
        let mut layout_lines = Vec::with_capacity(1);
        line.layout_to_buffer(
            &mut self.scratch,
            font_size.as_f32(),
            None,
            cosmic_text::Wrap::None,
            None,
            &mut layout_lines,
            None,
            cosmic_text::Hinting::Disabled,
        );
        let layout = layout_lines
            .first()
            .expect("layout should contain one line");

        let mut runs: Vec<ShapedRun> = Vec::new();
        for glyph in &layout.glyphs {
            let mut font_id = FontId(glyph.metadata);
            let mut loaded_font = self.loaded_font(font_id);
            if loaded_font.font.id() != glyph.font_id {
                font_id = self.font_id_for_cosmic_id(glyph.font_id);
                loaded_font = self.loaded_font(font_id);
            }
            let is_emoji = loaded_font.is_known_emoji_font;

            if glyph.glyph_id == 3 && is_emoji {
                continue;
            }

            let shaped_glyph = ShapedGlyph {
                id: GlyphId(glyph.glyph_id as u32),
                position: point(glyph.x.into(), glyph.y.into()),
                index: glyph.start,
                is_emoji,
            };

            if let Some(last_run) = runs
                .last_mut()
                .filter(|last_run| last_run.font_id == font_id)
            {
                last_run.glyphs.push(shaped_glyph);
            } else {
                runs.push(ShapedRun {
                    font_id,
                    glyphs: vec![shaped_glyph],
                });
            }
        }

        LineLayout {
            font_size,
            width: layout.w.into(),
            ascent: layout.max_ascent.into(),
            descent: layout.max_descent.into(),
            runs,
            len: text.len(),
        }
    }
}

fn cosmic_font_features_from(features: &FontFeatures) -> Result<CosmicFontFeatures> {
    let mut result = CosmicFontFeatures::new();
    for feature in features.0.iter() {
        let name_bytes: [u8; 4] = feature
            .0
            .as_bytes()
            .try_into()
            .context("incorrect feature flag format")?;

        let tag = cosmic_text::FeatureTag::new(&name_bytes);
        result.set(tag, feature.1);
    }
    Ok(result)
}

fn font_into_properties(font: &Font) -> font_kit::properties::Properties {
    font_kit::properties::Properties {
        style: match font.style {
            FontStyle::Normal => font_kit::properties::Style::Normal,
            FontStyle::Italic => font_kit::properties::Style::Italic,
            FontStyle::Oblique => font_kit::properties::Style::Oblique,
        },
        weight: font_kit::properties::Weight(font.weight.0),
        stretch: Default::default(),
    }
}

fn face_info_into_properties(
    face_info: &cosmic_text::fontdb::FaceInfo,
) -> font_kit::properties::Properties {
    font_kit::properties::Properties {
        style: match face_info.style {
            cosmic_text::Style::Normal => font_kit::properties::Style::Normal,
            cosmic_text::Style::Italic => font_kit::properties::Style::Italic,
            cosmic_text::Style::Oblique => font_kit::properties::Style::Oblique,
        },
        weight: font_kit::properties::Weight(face_info.weight.0.into()),
        stretch: match face_info.stretch {
            cosmic_text::Stretch::Condensed => font_kit::properties::Stretch::CONDENSED,
            cosmic_text::Stretch::Expanded => font_kit::properties::Stretch::EXPANDED,
            cosmic_text::Stretch::ExtraCondensed => font_kit::properties::Stretch::EXTRA_CONDENSED,
            cosmic_text::Stretch::ExtraExpanded => font_kit::properties::Stretch::EXTRA_EXPANDED,
            cosmic_text::Stretch::Normal => font_kit::properties::Stretch::NORMAL,
            cosmic_text::Stretch::SemiCondensed => font_kit::properties::Stretch::SEMI_CONDENSED,
            cosmic_text::Stretch::SemiExpanded => font_kit::properties::Stretch::SEMI_EXPANDED,
            cosmic_text::Stretch::UltraCondensed => font_kit::properties::Stretch::ULTRA_CONDENSED,
            cosmic_text::Stretch::UltraExpanded => font_kit::properties::Stretch::ULTRA_EXPANDED,
        },
    }
}

fn check_is_known_emoji_font(postscript_name: &str) -> bool {
    postscript_name == "NotoColorEmoji"
}
