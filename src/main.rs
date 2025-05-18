use eframe::egui;
use screenshots::Screen;
use image::DynamicImage;
use image::GenericImageView;
use arboard::Clipboard;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;

struct CaptureApp {
    start_pos: Option<egui::Pos2>,
    current_pos: Option<egui::Pos2>,
    screenshot: Arc<Mutex<Option<DynamicImage>>>,
    has_captured: bool,
    // 添加原始屏幕图像
    original_screen: Option<Arc<Mutex<Option<DynamicImage>>>>,
    // 放大镜半径
    magnifier_radius: f32,
    // 放大倍数
    magnifier_scale: f32,
    // 放大镜位置偏移修正值
    magnifier_offset_x: f32,
    magnifier_offset_y: f32,
}

impl Default for CaptureApp {
    fn default() -> Self {
        // 在程序启动时截取整个屏幕
        let original_screen = Arc::new(Mutex::new(None));
        let screen_clone = original_screen.clone();
        
        // 在单独的线程中截取整个屏幕，避免UI渲染的干扰
        thread::spawn(move || {
            if let Ok(screens) = Screen::all() {
                if !screens.is_empty() {
                    let screen = &screens[0]; // 默认使用主屏幕
                    if let Ok(img) = screen.capture() {
                        let dynamic_img = DynamicImage::ImageRgba8(img);
                        *screen_clone.lock().unwrap() = Some(dynamic_img);
                    }
                }
            }
            // 给足够的时间确保截图完成
            thread::sleep(Duration::from_millis(100));
        });

        Self {
            start_pos: None,
            current_pos: None,
            screenshot: Arc::new(Mutex::new(None)),
            has_captured: false,
            original_screen: Some(original_screen),
            magnifier_radius: 80.0,
            magnifier_scale: 4.0,
            magnifier_offset_x: 0.0,
            magnifier_offset_y: 0.0,
        }
    }
}

impl eframe::App for CaptureApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 设置鼠标光标为十字形
        ctx.output_mut(|o| o.cursor_icon = egui::CursorIcon::Crosshair);
        
        // 设置背景为完全透明
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
            .show(ctx, |_| {});

        // 获取当前鼠标位置
        let current_mouse_pos = ctx.input(|i| i.pointer.hover_pos()).unwrap_or(egui::pos2(0.0, 0.0));
        
        // 全屏透明背景捕获鼠标事件
        egui::Area::new("fullscreen")
            .show(ctx, |ui| {
                ui.allocate_response(ui.available_size(), egui::Sense::click_and_drag());
            });

        // 检测ESC键或鼠标右键
        let esc_pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let right_clicked = ctx.input(|i| i.pointer.secondary_clicked());

        if esc_pressed || right_clicked {
            // 取消截图并关闭窗口
            self.start_pos = None;
            self.current_pos = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // 鼠标事件处理
        let is_primary_down = ctx.input(|i| i.pointer.primary_down());
        let interact_pos = ctx.input(|i| i.pointer.interact_pos());
        
        if is_primary_down {
            if self.start_pos.is_none() {
                self.start_pos = interact_pos;
            }
            self.current_pos = interact_pos;
        } else if self.start_pos.is_some() && !self.has_captured {
            self.has_captured = true;
            // 从已保存的屏幕截图中提取选定区域
            self.extract_selection(ctx);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // 绘制选择框
        if let (Some(start), Some(current)) = (self.start_pos, self.current_pos) {
            let rect = egui::Rect::from_two_pos(start, current);
            
            // 获取整个屏幕区域
            let screen_rect = ctx.screen_rect();
            
            let semi_transparent = egui::Color32::from_black_alpha(120);
            
            // 通过绘制四个矩形来覆盖除选中区域外的所有区域
            
            // 1. 上方区域
            let top_rect = egui::Rect::from_min_max(
                screen_rect.min,
                egui::pos2(screen_rect.max.x, rect.min.y)
            );
            ctx.layer_painter(egui::LayerId::background())
                .rect_filled(top_rect, 0.0, semi_transparent);
                
            // 2. 左侧区域
            let left_rect = egui::Rect::from_min_max(
                egui::pos2(screen_rect.min.x, rect.min.y),
                egui::pos2(rect.min.x, rect.max.y)
            );
            ctx.layer_painter(egui::LayerId::background())
                .rect_filled(left_rect, 0.0, semi_transparent);
                
            // 3. 右侧区域
            let right_rect = egui::Rect::from_min_max(
                egui::pos2(rect.max.x, rect.min.y),
                egui::pos2(screen_rect.max.x, rect.max.y)
            );
            ctx.layer_painter(egui::LayerId::background())
                .rect_filled(right_rect, 0.0, semi_transparent);
                
            // 4. 下方区域
            let bottom_rect = egui::Rect::from_min_max(
                egui::pos2(screen_rect.min.x, rect.max.y),
                screen_rect.max
            );
            ctx.layer_painter(egui::LayerId::background())
                .rect_filled(bottom_rect, 0.0, semi_transparent);
            
            // 为选择区域绘制边框
            ctx.layer_painter(egui::LayerId::background())
                .rect_stroke(rect, 0.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
        }
        
        // 绘制放大镜
        self.draw_magnifier(ctx, current_mouse_pos);
    }
}

// 截图功能实现
impl CaptureApp {
    fn extract_selection(&self, ctx: &egui::Context) {
        if let (Some(start), Some(end)) = (self.start_pos, self.current_pos) {
            if let Some(ref original_screen) = self.original_screen {
                if let Some(original_img) = &*original_screen.lock().unwrap() {
                    let pixels_per_point = ctx.pixels_per_point();
                    
                    // 转换为物理像素坐标
                    let min_x = start.x.min(end.x) * pixels_per_point;
                    let min_y = start.y.min(end.y) * pixels_per_point;
                    let width = (end.x - start.x).abs() * pixels_per_point;
                    let height = (end.y - start.y).abs() * pixels_per_point;
                    
                    // 转换为整数
                    let x = min_x as u32;
                    let y = min_y as u32;
                    let w = width as u32;
                    let h = height as u32;
                    
                    // 从原始屏幕截图中裁剪出选定区域
                    let cropped = original_img.crop_imm(x, y, w, h);
                    
                    // 保存截图
                    *self.screenshot.lock().unwrap() = Some(cropped);
                    self.save_or_copy();
                } else {
                    // 如果没有原始截图，则使用系统API进行截图
                    self.capture_selection(ctx);
                }
            } else {
                // 如果没有原始截图，则使用系统API进行截图
                self.capture_selection(ctx);
            }
        }
    }
    
    fn capture_selection(&self, ctx: &egui::Context) {
        if let (Some(start), Some(end)) = (self.start_pos, self.current_pos) {
            let pixels_per_point = ctx.pixels_per_point();
            
            // 转换为物理像素坐标
            let min_x = start.x.min(end.x) * pixels_per_point;
            let min_y = start.y.min(end.y) * pixels_per_point;
            let width = (end.x - start.x).abs() * pixels_per_point;
            let height = (end.y - start.y).abs() * pixels_per_point;
            
            // 转换为整数
            let x = min_x as i32;
            let y = min_y as i32;
            let w = width as u32;
            let h = height as u32;

            // 使用系统API直接截取屏幕区域
            if let Ok(screen) = Screen::from_point(x, y) {
                if let Ok(img) = screen.capture_area(x, y, w, h) {
                    // 将 RgbaImage 转换为 DynamicImage
                    let dynamic_img = DynamicImage::ImageRgba8(img);
                    *self.screenshot.lock().unwrap() = Some(dynamic_img);
                    self.save_or_copy();
                }
            }
        }
    }

    fn save_or_copy(&self) {
        let screenshot: Option<DynamicImage> = self.screenshot.lock().unwrap().take();
        if let Some(img) = screenshot {
            // 复制到剪贴板
            if let Ok(mut clipboard) = Clipboard::new() {
                let image_data = arboard::ImageData {
                    width: img.width() as usize,
                    height: img.height() as usize,
                    bytes: img.as_bytes().into(),
                };
                clipboard.set_image(image_data).ok();
            }

            // 弹出保存对话框
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name("screenshot.png")
                .save_file()
            {
                img.save(path).expect("Failed to save image");
            }
        }
    }
}

// 放大镜相关实现
impl CaptureApp {
    // 添加绘制放大镜的方法
    fn draw_magnifier(&self, ctx: &egui::Context, mouse_pos: egui::Pos2) {
        // 获取原始屏幕图像
        if let Some(ref original_screen) = self.original_screen {
            if let Some(ref original_img) = *original_screen.lock().unwrap() {
                let pixels_per_point = ctx.pixels_per_point();
                
                // 计算放大镜的大小
                let square_size = self.magnifier_radius * 2.0;
                
                // 获取屏幕尺寸
                let screen_rect = ctx.screen_rect();
                
                // 默认放大镜位置（鼠标位置右上方）
                let mut magnifier_x = mouse_pos.x + self.magnifier_radius * 1.2;
                let mut magnifier_y = mouse_pos.y - self.magnifier_radius * 1.2;
                
                // 检测是否超出右边界
                if magnifier_x + self.magnifier_radius > screen_rect.max.x {
                    // 如果超出右边界，放置在鼠标左侧
                    // magnifier_x = mouse_pos.x - self.magnifier_radius * 1.0 - self.magnifier_radius;
                    magnifier_x = mouse_pos.x - self.magnifier_radius * 1.2;
                }
                
                // 检测是否超出左边界
                if magnifier_x - self.magnifier_radius < screen_rect.min.x {
                    // 如果超出左边界，放置在鼠标右侧
                    magnifier_x = mouse_pos.x + self.magnifier_radius * 1.2;
                }
                
                // 检测是否超出上边界
                if magnifier_y - self.magnifier_radius < screen_rect.min.y {
                    // 如果超出上边界，放置在鼠标下方
                    magnifier_y = mouse_pos.y + self.magnifier_radius * 1.2;
                }
                
                // 检测是否超出下边界
                if magnifier_y + self.magnifier_radius > screen_rect.max.y {
                    // 如果超出下边界，放置在鼠标上方
                    magnifier_y = mouse_pos.y - self.magnifier_radius * 1.2 - self.magnifier_radius;
                }
                
                // 最终确定放大镜中心位置
                let magnifier_center = egui::pos2(magnifier_x, magnifier_y);
                
                // 计算方形放大镜的矩形
                let magnifier_rect = egui::Rect::from_center_size(
                    magnifier_center,
                    egui::vec2(square_size, square_size)
                );
                
                // 实现放大效果 - 获取鼠标位置对应的图像区域
                let mouse_x = (mouse_pos.x * pixels_per_point) as u32;
                let mouse_y = (mouse_pos.y * pixels_per_point) as u32;
                
                // 确保鼠标坐标在图像范围内
                if mouse_x < original_img.width() && mouse_y < original_img.height() {
                    // 在放大镜中绘制放大后的鼠标周围区域
                    // 绘制多个像素点来实现放大效果
                    let sample_radius = (self.magnifier_radius / self.magnifier_scale) as u32;
                    
                    // 确定采样区域边界
                    let sample_start_x = mouse_x.saturating_sub(sample_radius);
                    let sample_start_y = mouse_y.saturating_sub(sample_radius);
                    let sample_end_x = (mouse_x + sample_radius).min(original_img.width());
                    let sample_end_y = (mouse_y + sample_radius).min(original_img.height());
                    
                    // 用于剪切方形区域的辅助函数
                    let in_square = |px: f32, py: f32| -> bool {
                        px >= magnifier_rect.min.x && px <= magnifier_rect.max.x &&
                        py >= magnifier_rect.min.y && py <= magnifier_rect.max.y
                    };
                    
                    // 在放大镜中绘制放大后的图像
                    for y in sample_start_y..sample_end_y {
                        for x in sample_start_x..sample_end_x {
                            // 获取原始图像中的像素
                            let pixel = original_img.get_pixel(x, y);
                            
                            // 计算放大后在放大镜中的位置，纠正偏移，确保放大镜中心与鼠标位置对应像素对齐
                            let dx = x as f32 - mouse_x as f32;
                            let dy = y as f32 - mouse_y as f32;
                            let magnified_x = magnifier_center.x + dx * self.magnifier_scale + self.magnifier_offset_x;
                            let magnified_y = magnifier_center.y + dy * self.magnifier_scale + self.magnifier_offset_y;
                            
                            // 只绘制放大镜方形区域内的像素
                            if in_square(magnified_x, magnified_y) {
                                // 确定放大后的像素大小
                                let pixel_size = self.magnifier_scale.max(1.0);
                                
                                // 绘制放大后的像素点
                                ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("magnifier_pixels")))
                                    .rect_filled(
                                        egui::Rect::from_center_size(
                                            egui::pos2(magnified_x, magnified_y),
                                            egui::vec2(pixel_size, pixel_size)
                                        ),
                                        0.0,
                                        egui::Color32::from_rgba_unmultiplied(
                                            pixel[0], pixel[1], pixel[2], pixel[3]
                                        )
                                    );
                            }
                        }
                    }
                }
                
                // 定义十字线的正确中心点
                let cross_center = magnifier_center;
                
                // 绘制放大镜边框（使用透明填充物和边框）
                let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("magnifier_border")));
                
                // 先绘制边框
                painter.rect_stroke(
                    magnifier_rect,
                    0.0, // 圆角半径为0，即方形
                    egui::Stroke::new(2.0, egui::Color32::DARK_GRAY)
                );
                
                // 绘制十字线 - 使用较高的层级确保它显示在放大的像素上方，不使用背景色
                let line_stroke = egui::Stroke::new(1.5, egui::Color32::RED);
                
                // 水平线
                ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("magnifier_crosshair")))
                    .line_segment(
                        [
                            egui::pos2(magnifier_rect.min.x, cross_center.y),
                            egui::pos2(magnifier_rect.max.x, cross_center.y)
                        ],
                        line_stroke
                    );
                    
                // 垂直线
                ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("magnifier_crosshair")))
                    .line_segment(
                        [
                            egui::pos2(cross_center.x, magnifier_rect.min.y),
                            egui::pos2(cross_center.x, magnifier_rect.max.y)
                        ],
                        line_stroke
                    );
                
                // 在十字线交叉点绘制一个小圆点
                ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("magnifier_crosshair")))
                    .circle_filled(
                        cross_center,
                        3.0,
                        egui::Color32::from_rgb(255, 255, 0) // 黄色，更容易被看到
                    );
                
                // 提示文字：显示光标位置坐标
                let text = format!("({}, {})", 
                    (mouse_pos.x * pixels_per_point) as i32,
                    (mouse_pos.y * pixels_per_point) as i32
                );
                
                ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("magnifier")))
                    .text(
                        egui::pos2(magnifier_center.x, magnifier_rect.max.y + 15.0),
                        egui::Align2::CENTER_CENTER,
                        text,
                        egui::FontId::default(),
                        egui::Color32::WHITE
                    );
            }
        }
    }
}

fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1920.0, 1080.0])
            .with_transparent(true)
            .with_decorations(false)
            .with_fullscreen(true),
        ..Default::default()
    };
    
    let _ = eframe::run_native(
        "Rust Screen Capture",
        options,
        Box::new(|_cc| Box::new(CaptureApp::default())),
    );
}