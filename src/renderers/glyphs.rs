use std::collections::HashMap;
use std::mem::{offset_of, size_of};
use std::rc::Rc;
use std::sync::LazyLock;

use freetype::{Library, face::LoadFlag};
use glow::HasContext;
use rustybuzz::{Face as RbFace, ShapePlan, UnicodeBuffer};

use crate::font_registry::FontRegistry;
use crate::macros::macs::include_shader;
use crate::text_manager::{TermGlyph, TextManager};

static FT_LIB: LazyLock<Library> =
    LazyLock::new(|| Library::init().expect("failed to initialize FreeType library"));

pub struct ShapedGlyph {
    pub glyph_id: u32,
    // x_advance/y_advance are provided for completeness (callers doing free-flow layout
    // may use them); the terminal renderer ignores them in favour of a fixed cell width.
    #[allow(dead_code)]
    pub x_advance: f32,
    #[allow(dead_code)]
    pub y_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

struct GlyphTexture {
    tex: glow::NativeTexture,
    width: i32,
    height: i32,
    left: i32,
    top: i32,
}

pub struct TextRenderer<'a> {
    gl: Rc<glow::Context>,
    // font_data must outlive rb_face; both are dropped explicitly in Drop (rb_face first)
    font_registry: &'a FontRegistry,
    ft_face: freetype::Face,
    rb_face: RbFace<'a>,

    text_manager: TextManager,

    glyphs: HashMap<u32, GlyphTexture>,

    program: glow::NativeProgram,
    vao: glow::NativeVertexArray,
    vbo: glow::NativeBuffer,

    u_proj: Option<glow::NativeUniformLocation>,
    u_color: Option<glow::NativeUniformLocation>,
    u_background_color: Option<glow::NativeUniformLocation>,
    u_tex: Option<glow::NativeUniformLocation>,
    font_size_px: (f32, f32, f32), // (cell_width, cell_height, descender)
    px_size: f32,
    shape_plan: ShapePlan,
    shape_buffer: Option<UnicodeBuffer>,
}

impl<'a> TextRenderer<'a> {
    pub fn units_per_em(&self) -> f32 {
        self.rb_face.units_per_em() as f32
    }

    /// Returns (width, height) of the font at the current pixel size.
    /// This is not the same as the maximum glyph size, but can be used for layout purposes.
    pub fn font_size(&self) -> (f32, f32, f32) {
        self.font_size_px
    }

    pub fn update_font_size(&mut self, px_size: u32, dpi: u32) {
        self.ft_face
            .set_char_size(0, (px_size * 64) as isize, dpi, dpi)
            .expect("failed to set pixel size");

        let metrics = self
            .ft_face
            .size_metrics()
            .expect("failed to get size metrics");

        let cell_width = metrics.max_advance as f32 / 64.0;
        let cell_height = metrics.height as f32 / 64.0;
        let descender = metrics.descender as f32 / 64.0;

        self.font_size_px = (cell_width, cell_height, descender);
        self.px_size = px_size as f32;

        self.text_manager = TextManager::new(
            cell_width.ceil() as i32,
            cell_height.ceil() as i32,
            self.text_manager.window_width,
            self.text_manager.window_height,
            4,
        );

        self.glyphs.clear();
    }

    pub unsafe fn new(
        gl: Rc<glow::Context>,
        font_registry: &'a FontRegistry,
        px_size: u32,
        size: (i32, i32),
    ) -> Self {
        // Box the font bytes so they can be freed when the renderer is dropped.
        let font_data: &[u8] = font_registry
            .get_fonts()
            .first()
            .expect("no fonts registered")
            .bytes
            .as_slice();

        // SAFETY: we extend the lifetime to 'static here, but we guarantee that
        // rb_face (which borrows this data) is dropped before font_data in our Drop impl.
        let rb_face = RbFace::from_slice(font_data, 0).expect("failed to create rustybuzz face");

        let ft_face = FT_LIB
            .new_memory_face(font_data.to_vec(), 0)
            .expect("failed to load freetype face");
        ft_face
            .set_char_size(0, (px_size * 64) as isize, 96, 96)
            .expect("failed to set pixel size");

        let shape_plan = ShapePlan::new(
            &rb_face,
            rustybuzz::Direction::LeftToRight,
            Some(rustybuzz::script::LATIN),
            None,
            &[],
        );

        let metrics = ft_face.size_metrics().expect("failed to get size metrics");
        let cell_width = metrics.max_advance as f32 / 64.0;
        let cell_height = metrics.height as f32 / 64.0;
        let descender = metrics.descender as f32 / 64.0;
        let font_size_px = (cell_width, cell_height, descender);

        // Explicit unsafe block required by Rust 2024: unsafe fn bodies no longer
        // implicitly permit unsafe calls without an unsafe{} block.
        let program = unsafe { include_shader!(gl, "font") };

        let vao = unsafe { gl.create_vertex_array().unwrap() };
        let vbo = unsafe { gl.create_buffer().unwrap() };

        let u_proj = unsafe { gl.get_uniform_location(program, "u_proj") };
        let u_color = unsafe { gl.get_uniform_location(program, "u_text_color") };
        let u_background_color = unsafe { gl.get_uniform_location(program, "u_background_color") };
        let u_tex = unsafe { gl.get_uniform_location(program, "u_font") };

        let text_manager = TextManager::new(
            font_size_px.0.ceil() as i32,
            font_size_px.1.ceil() as i32,
            size.0,
            size.1,
            8,
        );

        Self {
            gl,
            font_registry,
            ft_face,
            rb_face,
            text_manager,
            glyphs: HashMap::new(),
            program,
            vao,
            vbo,
            u_proj,
            u_background_color,
            u_color,
            u_tex,
            font_size_px,
            px_size: px_size as f32,
            shape_plan,
            shape_buffer: Some(UnicodeBuffer::new()),
        }
    }

    pub fn clear_section(&self, x: i32, y: i32, width: i32, height: i32, color: [f32; 4]) {
        let y = self.text_manager.window_height - y; // Convert from top-left to bottom-left origin

        unsafe {
            let gl = self.gl.as_ref();
            gl.enable(glow::SCISSOR_TEST);

            gl.scissor(x, y, width, height);
            gl.clear_color(color[0], color[1], color[2], 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.disable(glow::SCISSOR_TEST);
        }
    }

    pub fn shape_text(&mut self, text: &str) -> Vec<ShapedGlyph> {
        let mut buffer = self
            .shape_buffer
            .take()
            .expect("shape_buffer already in use");

        buffer.push_str(text);

        let shaped = rustybuzz::shape_with_plan(&self.rb_face, &self.shape_plan, buffer);

        let infos = shaped.glyph_infos();
        let positions = shaped.glyph_positions();
        let glyphs = infos
            .iter()
            .zip(positions.iter())
            .map(|(info, pos)| ShapedGlyph {
                glyph_id: info.glyph_id,
                // rustybuzz positions are in font units; converted to pixels in draw_text
                x_advance: pos.x_advance as f32,
                y_advance: pos.y_advance as f32,
                x_offset: pos.x_offset as f32,
                y_offset: pos.y_offset as f32,
            })
            .collect();

        self.shape_buffer = Some(shaped.clear());

        glyphs
    }

    /// Ensures the glyph is loaded and cached.
    /// Returns a tuple of (left, top, width, height, tex) to avoid holding a
    /// reference into `self.glyphs` across subsequent `self` accesses.
    fn ensure_glyph(&mut self, glyph_id: u32) -> Option<(i32, i32, i32, i32, glow::NativeTexture)> {
        if !self.glyphs.contains_key(&glyph_id) {
            // SAFETY: requires an active GL context.
            let texture =
                unsafe { Self::load_glyph_texture(self.gl.as_ref(), &self.ft_face, glyph_id)? };
            self.glyphs.insert(glyph_id, texture);
        }
        let g = self.glyphs.get(&glyph_id)?;
        Some((g.left, g.top, g.width, g.height, g.tex))
    }

    /// Rasterises a single glyph with FreeType and uploads it to a GL texture.
    ///
    /// The texture uses the `GL_RED` internal format (one byte per texel) to
    /// minimise GPU memory usage.  The fragment shader samples only the red
    /// channel.
    unsafe fn load_glyph_texture(
        gl: &glow::Context,
        ft_face: &freetype::Face,
        glyph_id: u32,
    ) -> Option<GlyphTexture> {
        ft_face
            .load_glyph(
                glyph_id,
                LoadFlag::RENDER
                    | LoadFlag::TARGET_NORMAL
                    | LoadFlag::FORCE_AUTOHINT
                    | LoadFlag::TARGET_LCD,
            )
            .expect("freetype load_glyph failed");

        let slot = ft_face.glyph();
        let bitmap = slot.bitmap();

        let width = bitmap.width() / 3;
        let height = bitmap.rows();
        let left = slot.bitmap_left();
        let top = slot.bitmap_top();

        // Explicit unsafe block required by Rust 2024: unsafe fn bodies no longer
        // implicitly permit unsafe calls without an unsafe{} block.
        unsafe {
            let tex = gl.create_texture().ok()?;
            gl.bind_texture(glow::TEXTURE_2D, Some(tex));

            // Grayscale bitmap: one byte per pixel, no alignment padding needed.
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 3);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGB8 as i32,
                width,
                height,
                0,
                glow::RGB,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(bitmap.buffer())),
            );

            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );

            Some(GlyphTexture {
                tex,
                width,
                height,
                left,
                top,
            })
        }
    }

    pub unsafe fn draw_text(
        &mut self,
        text: &str,
        mut pen_x: f32,
        baseline_y: f32,
        color: [f32; 3],
        proj: &[f32; 16],
    ) -> usize {
        let shaped = self.shape_text(text);
        let _units_per_em = self.units_per_em();
        let _px_size = self.px_size;

        let stride = size_of::<Vertex>() as i32;
        let uv_offset = offset_of!(Vertex, uv) as i32;

        unsafe {
            let gl = self.gl.as_ref();

            gl.bind_vertex_array(Some(self.vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));

            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);

            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, uv_offset);

            gl.use_program(Some(self.program));

            gl.enable(glow::BLEND);
            gl.blend_func_separate(
                glow::SRC_ALPHA,
                glow::ONE_MINUS_SRC_ALPHA,
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
            );

            gl.uniform_matrix_4_f32_slice(self.u_proj.as_ref(), false, proj);
            gl.uniform_3_f32(self.u_color.as_ref(), color[0], color[1], color[2]);
            gl.uniform_1_i32(self.u_tex.as_ref(), 0);

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            for g in &shaped {
                // Extract all glyph data as owned/Copy values so the borrow on
                // `self.glyphs` ends before we re-access other fields of `self`.
                let Some((left, top, width, height, tex)) = self.ensure_glyph(g.glyph_id) else {
                    println!("Warning: glyph ID {} not found in font", g.glyph_id);
                    continue;
                };

                // let x_offset = hb_to_px(g.x_offset, px_size, units_per_em);
                // let y_offset = hb_to_px(g.y_offset, px_size, units_per_em);
                let x_offset = g.x_offset;
                let y_offset = g.y_offset;

                let x = pen_x + x_offset + left as f32;
                let y = baseline_y - y_offset - top as f32;

                let w = width as f32;
                let h = height as f32;

                if w > 0.0 && h > 0.0 {
                    let vertices = [
                        Vertex {
                            pos: [x, y],
                            uv: [0.0, 0.0],
                        },
                        Vertex {
                            pos: [x + w, y],
                            uv: [1.0, 0.0],
                        },
                        Vertex {
                            pos: [x + w, y + h],
                            uv: [1.0, 1.0],
                        },
                        Vertex {
                            pos: [x, y],
                            uv: [0.0, 0.0],
                        },
                        Vertex {
                            pos: [x + w, y + h],
                            uv: [1.0, 1.0],
                        },
                        Vertex {
                            pos: [x, y + h],
                            uv: [0.0, 1.0],
                        },
                    ];

                    let gl = self.gl.as_ref();
                    gl.active_texture(glow::TEXTURE0);
                    gl.bind_texture(glow::TEXTURE_2D, Some(tex));

                    gl.buffer_data_u8_slice(
                        glow::ARRAY_BUFFER,
                        bytemuck::cast_slice(&vertices),
                        glow::DYNAMIC_DRAW,
                    );

                    gl.draw_arrays(glow::TRIANGLES, 0, 6);
                }

                // NOTE: due to the way text is rendered in a terminal (in a fixed grid),
                // we ignore the actual x_advance and just move the pen by the cell width.
                pen_x += self.font_size_px.0;
            }

            let gl = self.gl.as_ref();
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            gl.use_program(None);

            shaped.len()
        }
    }

    pub unsafe fn draw_glyphs_bg(&self, glyphs: &[TermGlyph], row: i32, col: i32) {
        for (i, g) in glyphs.iter().enumerate() {
            let cell_box = self.text_manager.get_cell_box(row, col + i as i32);
            let bg_color = [
                g.bg_color.0 as f32 / 255.0,
                g.bg_color.1 as f32 / 255.0,
                g.bg_color.2 as f32 / 255.0,
            ];
            self.clear_section(
                cell_box.x,
                cell_box.y,
                cell_box.width,
                cell_box.height,
                [bg_color[0], bg_color[1], bg_color[2], 1.0],
            );
        }
    }

    pub unsafe fn draw_glyphs(
        &mut self,
        glyphs: &[TermGlyph],
        row: i32,
        col: i32,
        proj: &[f32; 16],
    ) {
        let cell_box = self.text_manager.get_cell_box(row, col);

        let mut pen_x: f32 = cell_box.x as f32;
        let baseline_y: f32 = cell_box.y as f32;

        let text = glyphs.iter().map(|g| g.char).collect::<String>();
        let shaped = self.shape_text(&text);
        let units_per_em = self.units_per_em();
        let px_size = self.px_size;

        let stride = size_of::<Vertex>() as i32;
        let uv_offset = offset_of!(Vertex, uv) as i32;

        let glyphs_iter = glyphs.iter().zip(shaped.iter());

        unsafe {
            self.draw_glyphs_bg(glyphs, row, col);
            let gl = self.gl.as_ref();

            gl.bind_vertex_array(Some(self.vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));

            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);

            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, uv_offset);

            gl.use_program(Some(self.program));

            gl.enable(glow::BLEND);
            gl.blend_func_separate(
                glow::SRC_ALPHA,
                glow::ONE_MINUS_SRC_ALPHA,
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
            );

            gl.uniform_matrix_4_f32_slice(self.u_proj.as_ref(), false, proj);
            gl.uniform_1_i32(self.u_tex.as_ref(), 0);

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            for (term_g, shaped_g) in glyphs_iter {
                // Extract all glyph data as owned/Copy values so the borrow on
                // `self.glyphs` ends before we re-access other fields of `self`.
                let Some((left, top, width, height, tex)) = self.ensure_glyph(shaped_g.glyph_id)
                else {
                    println!("Warning: glyph ID {} not found in font", shaped_g.glyph_id);
                    continue;
                };

                let x_offset = hb_to_px(shaped_g.x_offset, px_size, units_per_em);
                let y_offset = hb_to_px(shaped_g.y_offset, px_size, units_per_em);

                let w = width as f32;
                let h = height as f32;

                let x = pen_x + x_offset + left as f32;
                let y = baseline_y - y_offset - top as f32 + self.font_size_px.2; // Adjust for descender

                let fg_color = [
                    term_g.fg_color.0 as f32 / 255.0,
                    term_g.fg_color.1 as f32 / 255.0,
                    term_g.fg_color.2 as f32 / 255.0,
                ];
                let bg_color = [
                    term_g.bg_color.0 as f32 / 255.0,
                    term_g.bg_color.1 as f32 / 255.0,
                    term_g.bg_color.2 as f32 / 255.0,
                ];

                if w > 0.0 && h > 0.0 {
                    let vertices = [
                        Vertex {
                            pos: [x, y],
                            uv: [0.0, 0.0],
                        },
                        Vertex {
                            pos: [x + w, y],
                            uv: [1.0, 0.0],
                        },
                        Vertex {
                            pos: [x + w, y + h],
                            uv: [1.0, 1.0],
                        },
                        Vertex {
                            pos: [x, y],
                            uv: [0.0, 0.0],
                        },
                        Vertex {
                            pos: [x + w, y + h],
                            uv: [1.0, 1.0],
                        },
                        Vertex {
                            pos: [x, y + h],
                            uv: [0.0, 1.0],
                        },
                    ];
                    let gl = self.gl.as_ref();

                    gl.uniform_3_f32(self.u_color.as_ref(), fg_color[0], fg_color[1], fg_color[2]);
                    gl.uniform_3_f32(
                        self.u_background_color.as_ref(),
                        bg_color[0],
                        bg_color[1],
                        bg_color[2],
                    );
                    gl.active_texture(glow::TEXTURE0);
                    gl.bind_texture(glow::TEXTURE_2D, Some(tex));

                    gl.buffer_data_u8_slice(
                        glow::ARRAY_BUFFER,
                        bytemuck::cast_slice(&vertices),
                        glow::DYNAMIC_DRAW,
                    );

                    // FIX:
                    // Limit drawing to the row to prevent glyphs from bleeding into adjacent rows
                    // while allowing ligatures and diacritics to render correctly
                    // within neighbouring cells.
                    gl.enable(glow::SCISSOR_TEST);
                    gl.scissor(
                        0,
                        self.text_manager.window_height - cell_box.y,
                        self.text_manager.window_width,
                        cell_box.height,
                    );

                    gl.draw_arrays(glow::TRIANGLES, 0, 6);
                    gl.disable(glow::SCISSOR_TEST);
                }

                // NOTE: due to the way text is rendered in a terminal (in a fixed grid),
                // we ignore the actual x_advance and just move the pen by the cell width.
                pen_x += self.font_size_px.0;
            }

            let gl = self.gl.as_ref();
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            gl.use_program(None);
        }
    }

    pub fn set_viewport(&mut self, width: i32, height: i32) {
        self.text_manager.set_window_size(width, height);
    }
}

impl<'a> Drop for TextRenderer<'a> {
    fn drop(&mut self) {
        unsafe {
            let gl = self.gl.as_ref();

            for glyph in self.glyphs.values() {
                gl.delete_texture(glyph.tex);
            }

            gl.delete_vertex_array(self.vao);
            gl.delete_buffer(self.vbo);
            gl.delete_program(self.program);

            // Drop rb_face before releasing the font data it references.
            // ManuallyDrop::drop(&mut self.rb_face);
            // ManuallyDrop::drop(&mut self._font_data);
        }
    }
}

fn hb_to_px(v: f32, px_size: f32, units_per_em: f32) -> f32 {
    v * (px_size / units_per_em)
}
