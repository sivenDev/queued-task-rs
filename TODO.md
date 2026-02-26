# 优化 TODO List

## 代码优化

- [ ] **简化 `Shared` 的 Clone 实现**：使用 `#[derive(Clone)]` 替代手动实现
- [ ] **清理注释代码**：删除 `lib.rs` 中被注释的 `Config` 结构体（73-86行）
- [ ] **改进 `build()` 错误处理**：`handle.take().unwrap()` 会在未设置 handle 时 panic，建议改为返回 `Result` 或使用 builder pattern 强制设置
- [ ] **简化 `drop(p)`**：移除显式 drop 调用，让变量自然离开作用域即可

## API 增强

- [ ] **为 `QueuedTask` 实现 `Clone`**：方便多处共享队列引用
- [ ] **添加 `try_push()` 方法**：非阻塞推送，队列满时立即返回错误
- [ ] **添加 `is_closed()` 方法**：检查队列是否已关闭
- [ ] **添加 `max_capacity()` 方法**：获取队列最大容量（当前只有 `capacity()` 返回剩余容量）

## 文档修复

- [ ] **同步 README 版本号**：README 中安装示例是 `0.1.0`，Cargo.toml 是 `0.1.5`
- [ ] **添加 API 文档注释**：为公开的结构体和方法添加 `///` 文档注释

## 测试完善

- [ ] **添加超时推送测试**：测试 `push_timeout()` 功能
- [ ] **添加错误处理测试**：测试队列关闭后的行为
- [ ] **添加并发边界测试**：测试高并发场景下的稳定性

## 性能优化

- [ ] **考虑使用 `oneshot` 替代 `Shared`**：`tokio::sync::oneshot` 更适合单次结果传递的场景，比 `Mutex<Option<R>>` + `Notify` 更轻量
