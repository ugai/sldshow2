use anyhow::Result;
use glyphon::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer as GlyphonTextRenderer,
};
use wgpu::{Device, MultisampleState, Queue, RenderPass, SurfaceConfiguration};

pub struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: ValidationViewport,
    atlas: TextAtlas,
    text_renderer: GlyphonTextRenderer,
    pub buffer: Buffer,
    preferred_font_family: Option<String>,
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
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(20.0, 25.0));

        buffer.set_size(&mut font_system, config.width as f32, config.height as f32);

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
            preferred_font_family: font_family.map(|s| s.to_string()),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport.width = width;
        self.viewport.height = height;
        self.buffer
            .set_size(&mut self.font_system, width as f32, height as f32);
        self.buffer.shape_until_scroll(&mut self.font_system);
    }

    pub fn set_text(&mut self, text: &str) {
        // Determine attributes from the preferred font if it exists
        let mut family = Family::SansSerif;
        let mut weight = glyphon::Weight::NORMAL;
        let mut stretch = glyphon::Stretch::Normal;
        let mut style = glyphon::Style::Normal;

        if let Some(ref name) = self.preferred_font_family {
            family = Family::Name(name);

            // Query to find the actual attributes of this font in the DB
            let query = glyphon::fontdb::Query {
                families: &[Family::Name(name)],
                weight: glyphon::Weight::NORMAL, // We query for normal, but we will accept whatever we find
                stretch: glyphon::Stretch::Normal,
                style: glyphon::Style::Normal,
            };

            // If we find it, use ITS attributes to ensure the best match
            if let Some(id) = self.font_system.db().query(&query) {
                if let Some(face) = self.font_system.db().face(id) {
                    weight = face.weight;
                    style = face.style;
                    stretch = face.stretch;
                }
            }
        }

        self.buffer.set_text(
            &mut self.font_system,
            text,
            Attrs::new()
                .family(family)
                .weight(weight)
                .style(style)
                .stretch(stretch)
                .color(Color::rgb(255, 255, 255)),
            Shaping::Advanced,
        );
        self.buffer.shape_until_scroll(&mut self.font_system);
    }

    pub fn render<'a>(
        &'a mut self,
        device: &Device,
        queue: &Queue,
        pass: &mut RenderPass<'a>,
    ) -> Result<()> {
        self.text_renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            Resolution {
                width: self.viewport.width,
                height: self.viewport.height,
            },
            [TextArea {
                buffer: &self.buffer,
                left: 10.0,
                top: 10.0,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: self.viewport.width as i32,
                    bottom: self.viewport.height as i32,
                },
                default_color: Color::rgb(255, 255, 255),
            }],
            &mut self.swash_cache,
        )?;

        self.text_renderer
            .render(&self.atlas, pass)
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }
}
