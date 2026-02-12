//! Text rendering via glyphon/cosmic-text with multiple display areas.

use anyhow::Result;
use glyphon::cosmic_text::Align;
use glyphon::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer as GlyphonTextRenderer,
};
use wgpu::{Device, MultisampleState, Queue, RenderPass, SurfaceConfiguration};

/// Line height as a multiple of font size
const LINE_HEIGHT_RATIO: f32 = 1.25;
/// Vertical offset for main and OSD text areas (in font-size units)
const TEXT_MARGIN_TOP: f32 = 0.5;
/// Vertical offset for info overlay text area (in font-size units)
const INFO_OFFSET_TOP: f32 = 2.5;

pub struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: ValidationViewport,
    atlas: TextAtlas,
    text_renderer: GlyphonTextRenderer,
    buffer: Buffer,
    osd_buffer: Buffer,
    info_buffer: Buffer,
    preferred_font_family: Option<String>,
    current_color: Color,
    current_font_size: f32,
    show_info_overlay: bool,
}

enum BufferTarget {
    Main,
    Osd,
    Info,
}

struct ValidationViewport {
    width: u32,
    height: u32,
}

impl TextRenderer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        config: &SurfaceConfiguration,
        font_family: Option<&str>,
    ) -> Result<Self> {
        let mut font_system = FontSystem::new();

        // Load custom fonts from assets/fonts
        // Search paths: relative to executable, then current working directory
        let search_paths = [
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.join("assets").join("fonts"))),
            Some(std::path::PathBuf::from("assets/fonts")),
        ];

        let mut fonts_loaded = false;
        for path in search_paths.iter().flatten() {
            if path.exists() {
                log::info!("Loading fonts from: {:?}", path);
                font_system.db_mut().load_fonts_dir(path);
                fonts_loaded = true;
                // We don't break here because we might want to load from multiple locations?
                // Or maybe the first one is enough. Let's load from all valid locations to be safe.
            }
        }

        if !fonts_loaded {
            log::warn!("No assets/fonts directory found in search paths.");
        }

        // Check if requested font family exists
        if let Some(name) = font_family {
            let mut found = false;
            for face in font_system.db().faces() {
                for (family, _) in face.families.iter() {
                    if family == name {
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
            }

            if found {
                log::info!("Found requested font family: '{}'", name);
            } else {
                log::warn!(
                    "Requested font family '{}' not found. Falling back to default system font.",
                    name
                );
            }
        }

        let swash_cache = SwashCache::new();
        let viewport = ValidationViewport {
            width: config.width,
            height: config.height,
        };
        let mut atlas = TextAtlas::new(device, queue, config.format);
        let text_renderer =
            GlyphonTextRenderer::new(&mut atlas, device, MultisampleState::default(), None);
        let mut buffer = Buffer::new(
            &mut font_system,
            Metrics::new(20.0, 20.0 * LINE_HEIGHT_RATIO),
        );
        let mut osd_buffer = Buffer::new(
            &mut font_system,
            Metrics::new(20.0, 20.0 * LINE_HEIGHT_RATIO),
        );
        let mut info_buffer = Buffer::new(
            &mut font_system,
            Metrics::new(20.0, 20.0 * LINE_HEIGHT_RATIO),
        );

        buffer.set_size(&mut font_system, config.width as f32, config.height as f32);
        osd_buffer.set_size(
            &mut font_system,
            config.width as f32 - 20.0,
            config.height as f32,
        );
        info_buffer.set_size(&mut font_system, config.width as f32, config.height as f32);

        // Note: glyphon::Attributes::family() takes a Family enum.
        // If we provide Family::Name("FoundName"), it tries to use it.
        // If it was not found in the DB, glyphon will handle fallback internally during shaping/rasterization,
        // but we have already warned the user above.
        let family = if let Some(name) = font_family {
            Family::Name(name)
        } else {
            Family::SansSerif
        };

        buffer.set_text(
            &mut font_system,
            "",
            Attrs::new().family(family),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut font_system);

        Ok(Self {
            font_system,
            swash_cache,
            viewport,
            atlas,
            text_renderer,
            buffer,
            osd_buffer,
            info_buffer,
            preferred_font_family: font_family.map(|s| s.to_string()),
            current_color: Color::rgb(255, 255, 255),
            current_font_size: 20.0,
            show_info_overlay: false,
        })
    }

    fn resolve_attributes(
        db: &glyphon::fontdb::Database,
        family_name: Option<&str>,
    ) -> (glyphon::Weight, glyphon::Stretch, glyphon::Style) {
        let mut weight = glyphon::Weight::NORMAL;
        let mut stretch = glyphon::Stretch::Normal;
        let mut style = glyphon::Style::Normal;

        if let Some(name) = family_name {
            let query = glyphon::fontdb::Query {
                families: &[Family::Name(name)],
                weight: glyphon::Weight::NORMAL,
                stretch: glyphon::Stretch::Normal,
                style: glyphon::Style::Normal,
            };

            if let Some(id) = db.query(&query) {
                if let Some(face) = db.face(id) {
                    weight = face.weight;
                    style = face.style;
                    stretch = face.stretch;
                }
            }
        }
        (weight, stretch, style)
    }

    pub fn set_style(&mut self, font_size: f32, color_rgba: [u8; 4]) {
        self.current_font_size = font_size;
        self.current_color =
            Color::rgba(color_rgba[0], color_rgba[1], color_rgba[2], color_rgba[3]);
        // Force buffer functionality update
        let metrics = Metrics::new(font_size, font_size * LINE_HEIGHT_RATIO);
        self.buffer.set_metrics(&mut self.font_system, metrics);
        self.osd_buffer.set_metrics(&mut self.font_system, metrics);
        self.info_buffer.set_metrics(&mut self.font_system, metrics);

        self.osd_buffer.set_size(
            &mut self.font_system,
            self.viewport.width as f32 - self.current_font_size,
            self.viewport.height as f32,
        );
        self.osd_buffer.shape_until_scroll(&mut self.font_system);
        self.info_buffer.set_size(
            &mut self.font_system,
            self.viewport.width as f32,
            self.viewport.height as f32,
        );
        self.info_buffer.shape_until_scroll(&mut self.font_system);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport.width = width;
        self.viewport.height = height;
        self.buffer
            .set_size(&mut self.font_system, width as f32, height as f32);
        self.buffer.shape_until_scroll(&mut self.font_system);
        self.osd_buffer.set_size(
            &mut self.font_system,
            width as f32 - self.current_font_size,
            height as f32,
        );
        self.osd_buffer.shape_until_scroll(&mut self.font_system);
        self.info_buffer
            .set_size(&mut self.font_system, width as f32, height as f32);
        self.info_buffer.shape_until_scroll(&mut self.font_system);
    }

    fn set_buffer_text(&mut self, buffer: BufferTarget, text: &str) {
        let (weight, stretch, style) =
            Self::resolve_attributes(self.font_system.db(), self.preferred_font_family.as_deref());
        let family = self
            .preferred_font_family
            .as_deref()
            .map(Family::Name)
            .unwrap_or(Family::SansSerif);

        let attrs = Attrs::new()
            .family(family)
            .weight(weight)
            .style(style)
            .stretch(stretch)
            .color(self.current_color);

        let buf = match buffer {
            BufferTarget::Main => &mut self.buffer,
            BufferTarget::Osd => &mut self.osd_buffer,
            BufferTarget::Info => &mut self.info_buffer,
        };

        buf.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);

        if matches!(buffer, BufferTarget::Osd) {
            for line in buf.lines.iter_mut() {
                line.set_align(Some(Align::Right));
            }
        }

        buf.shape_until_scroll(&mut self.font_system);
    }

    pub fn set_text(&mut self, text: &str) {
        self.set_buffer_text(BufferTarget::Main, text);
    }

    pub fn set_osd_text(&mut self, text: &str) {
        self.set_buffer_text(BufferTarget::Osd, text);
    }

    pub fn set_info_text(&mut self, text: &str) {
        self.set_buffer_text(BufferTarget::Info, text);
    }

    pub fn toggle_info_overlay(&mut self) -> bool {
        self.show_info_overlay = !self.show_info_overlay;
        self.show_info_overlay
    }

    pub fn info_overlay_visible(&self) -> bool {
        self.show_info_overlay
    }

    /// Returns true if the info buffer has non-empty content to render.
    /// main.rs manages setting/clearing the buffer content based on overlay state and temporary messages.
    fn info_has_content(&self) -> bool {
        self.info_buffer
            .lines
            .iter()
            .any(|line| !line.text().is_empty())
    }

    pub fn render<'a>(
        &'a mut self,
        device: &Device,
        queue: &Queue,
        pass: &mut RenderPass<'a>,
    ) -> Result<()> {
        let mut text_areas = vec![
            TextArea {
                buffer: &self.buffer,
                left: self.current_font_size,
                top: self.current_font_size * TEXT_MARGIN_TOP,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: self.viewport.width as i32,
                    bottom: self.viewport.height as i32,
                },
                default_color: Color::rgb(255, 255, 255),
            },
            TextArea {
                buffer: &self.osd_buffer,
                left: 0.0,
                top: self.current_font_size * TEXT_MARGIN_TOP,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: self.viewport.width as i32,
                    bottom: self.viewport.height as i32,
                },
                default_color: Color::rgb(255, 255, 255),
            },
        ];

        // Show info buffer when persistent overlay is active OR temporary info message is set
        // (main.rs manages setting/clearing the buffer content)
        if self.info_has_content() {
            text_areas.push(TextArea {
                buffer: &self.info_buffer,
                left: self.current_font_size,
                top: self.current_font_size * INFO_OFFSET_TOP,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: self.viewport.width as i32,
                    bottom: self.viewport.height as i32,
                },
                default_color: Color::rgb(255, 255, 255),
            });
        }

        self.text_renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            Resolution {
                width: self.viewport.width,
                height: self.viewport.height,
            },
            text_areas,
            &mut self.swash_cache,
        )?;

        self.text_renderer
            .render(&self.atlas, pass)
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }
}
