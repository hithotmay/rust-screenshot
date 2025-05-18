# Rust截图工具

一个使用Rust编写的简单高效的屏幕截图工具，带有放大镜功能。

## 功能特点

- 全屏截图，支持区域选择
- 实时放大镜功能，帮助精确选择
- 自动复制到剪贴板
- 支持保存为PNG格式
- 放大镜自适应屏幕边缘显示
- 支持ESC或右键取消截图

## 技术栈

- Rust
- egui/eframe - UI框架
- screenshots - 屏幕捕获库
- image - 图像处理库
- arboard - 剪贴板操作库

## 使用方法

1. 启动程序
2. 使用鼠标左键拖动选择截图区域
3. 释放鼠标左键完成截图
4. 按ESC或右键取消操作

## 编译运行

```bash
cargo build --release
cargo run --release
```

## 许可证

MIT 