<div align="center">

# Meetily-ZH

**中文优先的 AI 会议助手 · 全程本地运行**

[English](README.en.md) · **中文**

[![Release](https://img.shields.io/github/v/release/kevinyuan/meetily-zh?color=brightgreen&label=%E6%9C%80%E6%96%B0%E7%89%88%E6%9C%AC)](https://github.com/kevinyuan/meetily-zh/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/kevinyuan/meetily-zh/total?color=blue&label=%E4%B8%8B%E8%BD%BD%E9%87%8F)](https://github.com/kevinyuan/meetily-zh/releases)
[![Stars](https://img.shields.io/github/stars/kevinyuan/meetily-zh?style=flat&color=yellow)](https://github.com/kevinyuan/meetily-zh/stargazers)
[![License](https://img.shields.io/badge/License-MIT-blue)](LICENSE.md)
[![Platform](https://img.shields.io/badge/%E5%B9%B3%E5%8F%B0-macOS_(Apple_Silicon)-lightgrey?logo=apple)](#安装--macos)
[![Engine](https://img.shields.io/badge/%E8%BD%AC%E5%BD%95%E5%BC%95%E6%93%8E-SenseVoice-orange)](https://github.com/FunAudioLLM/SenseVoice)
[![Built with](https://img.shields.io/badge/Tauri_+_Rust-24C8B8?logo=tauri&logoColor=white)](https://tauri.app)

**不上云 · 音频不出设备 · 无需订阅**

</div>

---

[**Meetily**](https://github.com/Zackriya-Solutions/meeting-minutes) 的中文优先分支 —— 一个隐私优先的 AI 会议助手，录音、转录、生成摘要**全部在你自己的电脑上完成**。不上云，音频不出设备。

上游有的功能这里都有。新增的，是**真正可用**的中文支持。

---

## 为什么会有这个分支

上游 Meetily 是个不错的项目，但它的转录和摘要链路默认假设你说英语。用它开中文会议，有三件事会出错，而且每一件都不是小毛病：

1. **说中文，转录出来是英文。** 默认的语言选项是 `auto-translate`：检测语言，然后**把它翻译成英文**。
2. **中文会议，摘要是英文。** 摘要在代码层面就是用英文起草的。选「自动」时，中文会议会被摘要后**再翻译成英文** —— 和这个选项承诺的正好相反。
3. **默认引擎根本不支持中文。** 引导流程默认安装 Parakeet，而它压根不能转录中文。

所以这个分支修正了默认行为，加入了为中文而生的转录引擎，并把界面翻译成了中文。

---

## 新增了什么

### SenseVoice —— 中文优先的转录引擎

[SenseVoice](https://github.com/FunAudioLLM/SenseVoice)（阿里通义 / FunAudioLLM）作为第三个引擎加入，并成为新的默认选项。它支持**中文、英文、日文、韩文、粤语**，在 CPU 上约 **60 倍实时速度**，并且输出标点。

模型用的是 sherpa-onnx 的官方 int8 导出版（约 236 MB），在 app 内下载。它复用 app 里已经为 Parakeet 准备的 ONNX Runtime，不额外打包任何东西。

### 按句断行的转录

以前，一行转录 = 两次超过 400ms 静音之间的所有内容。但正常语速下，**句子之间的停顿通常短于 400ms**，于是好几句话被粘成一行 —— 真实录音里出现过长达 **39 秒**的单行。

现在可以按**模型自己预测的标点**断句，时间戳来自它的声学（CTC）对齐。两种模式都能在**「设置 → 录音」**里切换，不支持标点断句的引擎会自动回退到旧行为，不会丢失任何文字。

### 逐句语言检测

SenseVoice 会单独识别每一句话的语言，所以中英夹杂的会议会**逐行标注**语言。

短句（不足 1 秒）的自动识别并不可靠 —— 实测中不到 0.5 秒的片段有 **54.8%** 会被识别错，而且识别错了不只是标签错，模型会**用错误的语言去解码**（中文的「对啦呢」被听成日文假名 `だらね`）。所以短句改用「本次会议的主导语言」解码，主导语言由那些足够长、可信的句子投票产生。

### 中文界面

整个界面已翻译（约 890 条文案）。显示语言默认根据系统区域自动检测，也可在**「设置 → 通用」**里手动切换。转录语言、摘要语言和模型筛选默认都跟随它。

### 中文优先的默认值

除非你明确要求，转录不再翻译成英文。摘要用界面语言撰写。模型列表会筛掉处理不了当前语言的引擎 —— 选中文时 Parakeet 会被隐藏，因为它确实做不到。

### 其他修复

- **找回被丢弃的语音。** 短于 250ms 的话会被直接丢掉，「嗯」「好」「对」/ "yes" / "OK" 就这样悄悄消失了。下限已降到 120ms。
- **修复音频混音。** 系统声音明明注释写着已衰减，实际却是满音量混入，随后的削波保护又把麦克风一起压了下去 —— 会议正在播放时，小声说话就检测不到。
- **VAD 录音中途崩溃。** Silero 的填充参数超出缓冲区时会 **panic 而不是报错**，直接打死音频管线：转录在开始几秒后彻底停止，而界面看上去还在录。
- **转录时间戳错误。** 长段落被强制切分后，时间戳会漂移到实际时长的约 2 倍。
- **给转录查询加索引。** 打开任何一个会议都会全表扫描 transcripts。
- **Markdown 导出。** 会议存在 SQLite 里，导出后可以放进文件夹、Git 或 iCloud。
- **macOS 菜单栏图标**改成了符合规范的单色模板图标，而不是直接拿彩色应用图标顶替。

---

## 安装 —— macOS

**仅支持 Apple Silicon（arm64）。**

### 方式一：下载 DMG（推荐）

从 [Releases](https://github.com/kevinyuan/meetily-zh/releases/latest) 下载 `meetily_*_aarch64.dmg`，打开后拖进「应用程序」。

> **首次启动请右键点击 → 打开。**
> 这个包没有 Apple 开发者签名和公证，Gatekeeper 会拦一次。之后就能正常双击打开了。

### 方式二：从源码构建

```bash
git clone https://github.com/kevinyuan/meetily-zh.git
cd meetily-zh/frontend
pnpm install
./dev.sh                  # 开发模式
pnpm run tauri:build      # 构建发布版
```

**构建前置条件**

- **Xcode** —— 需要完整的 Xcode，不是只装 Command Line Tools。依赖 `cidre`（macOS 系统声音采集）需要 `xcodebuild`。
- **cmake** —— `brew install cmake`，用于构建 whisper.cpp。

> 开发时请用 `./dev.sh` 启动，不要直接用 `pnpm run tauri:dev`。
> 后者会在 Next.js 编译完成前就打开窗口，导致 webview 读到不完整的 JS 包、React 无法完成水合 —— 表现为**界面能看见但完全点不动**。`dev.sh` 会先确认所有 chunk 完整可解析，再开窗口。

### 首次启动

会下载 SenseVoice 模型（约 236 MB）和摘要模型到：

```
~/Library/Application Support/com.meetily-zh.ai/models/
```

它们在各次构建和发布之间复用，不会重复下载。

macOS 会请求**麦克风**和**屏幕录制**权限。屏幕录制是用来采集通话中**对方**的声音的 —— 不给这个权限，你就只能录到自己。

> **从上游 Meetily 迁移过来的用户注意：** 本分支使用独立的应用数据目录（`com.meetily-zh.ai`），不会读取上游 `com.meetily.ai` 下的会议和模型。两者可以共存，互不干扰。

---

## Windows

**代码是支持 Windows 的，但这个分支没有在 Windows 上测试过。**

上游会构建并发布 Windows 安装包，而这里新增的东西没有任何 macOS 专属成分：SenseVoice、ONNX Runtime、断句逻辑和界面全都是跨平台的。仓库自带的工作流（`.github/workflows/build-windows.yml`）会在 GitHub 托管的 Windows runner 上构建 MSI 和 NSIS 安装包，且在没有代码签名证书的情况下也能成功（跳过签名而非失败）。

我没有 Windows 机器，也没有时间去测，所以不对它做任何保证。有两块需要真机验证：

- **音频采集**，Windows 走的是 WASAPI 而不是 CoreAudio。这里的混音和静音检测改动只在 macOS 上验证过，两者的缓冲行为并不相同。
- **系统声音采集**，那是完全独立的另一套实现。

如果你在 Windows 上构建并测试了，欢迎开 issue 告诉我结果。也欢迎 PR。

**Linux** 能编译，但上游也没有发布版本 —— 你需要自己从源码构建。

---

## 致谢

- [**Meetily**](https://github.com/Zackriya-Solutions/meeting-minutes)，作者 Zackriya Solutions —— 这个分支的全部基础。
- [**SenseVoice**](https://github.com/FunAudioLLM/SenseVoice)，来自阿里通义实验室，由 [**sherpa-onnx**](https://github.com/k2-fsa/sherpa-onnx)（新一代 Kaldi）导出为 ONNX。
- SenseVoice 的推理代码是 [**transcribe-rs**](https://crates.io/crates/transcribe-rs)（MIT）的一个 vendored 子集，回移植到了较旧版本的 ONNX Runtime —— 见 `frontend/src-tauri/src/sensevoice_engine/vendor/`。

MIT 协议，与上游一致。
