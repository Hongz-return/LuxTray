# LuxTray

轻量的 Windows 托盘亮度工具：外接显示器走 DDC/CI，笔记本内屏走 WMI。无 Electron，后台占用很低。

## 下载

最新版：<https://github.com/Hongz-return/LuxTray/releases/latest>

- `LuxTray.exe` — 单文件，拷走即可用
- `LuxTray-0.1.0-portable.zip` — 压缩包
- `LuxTray-0.1.0-setup.exe` — 安装包

Windows 可能提示「未经识别的应用」，选择仍要运行即可（未做代码签名）。

## 功能

- 左键托盘：弹出亮度面板（每台显示器 + 全部）
- 右键托盘：刷新、开机启动、快捷键、退出
- `Ctrl + Alt + ↑ / ↓`：所有显示器一起加减亮度
- 滚轮：在面板上滚动即可调节
- 睡眠唤醒后自动恢复上次亮度
- 滑条低于 0：硬件到最暗后再用软件压暗
- 命令行可脚本化

## 使用

```powershell
cargo run --release
```

双击运行后会出现在系统托盘（若被隐藏，点任务栏「显示隐藏的图标」）。

## 打包发布

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\package.ps1
```

产物在 `dist\`：

- `LuxTray.exe` — 单文件便携版，拷走即可用
- `LuxTray-0.1.0-portable.zip` — 压缩包
- `LuxTray-0.1.0-setup.exe` — 安装包（需本机已装 [Inno Setup 6](https://jrsoftware.org/isinfo.php)）

外接屏请在显示器 OSD 里打开 **DDC/CI**。部分 HDMI/扩展坞/KVM 会拦截该协议。

## 命令行

```text
luxtray                  启动托盘（已在运行则唤出面板）
luxtray list             列出显示器
luxtray set 50           全部设为 50%
luxtray set 70 --monitor 1
luxtray offset -10
luxtray offset 5 --monitor 2
```

## 快捷键 / 菜单

| 操作 | 说明 |
| --- | --- |
| 左键托盘 | 打开 / 关闭亮度面板 |
| 右键托盘 | 菜单 |
| Ctrl+Alt+↑ | 全部 +5% |
| Ctrl+Alt+↓ | 全部 -5% |

配置文件：`%APPDATA%\LuxTray\config.json`
