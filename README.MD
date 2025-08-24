# I am Frequency Remapper

一个基于 Phase Vocoder 的实时频率重映射器，允许用户将输入音频的频率通过自定义的 [Rhai](https://rhai.rs/) 脚本映射到新的频率, 基于 [nih_plug](https://github.com/robbert-vdh/nih-plug). 

## User Guide

### 安装

`x86_64-win` 平台下在 `Release` 中下载 vst3 文件后将其放到 `C:\Program Files\Common Files\VST3`（必须是这个路径).
其他平台可能需要你参考后续编译条目自行编译

### 使用

这里我们假定你熟悉或至少会用 `Rhai` 语言，这是一个类似 `rust` 的脚本语言，你可以在 [这里](https://rhai.rs/) 找到他的文档。

打开 vst 后点击 `load` / `加载` 即可加载在 `/Documents/mapper.rhai` 的 `rhai` 脚本，下面列举了插件提供的变量：

| 名称    | 变量名                | 备注                        | 类型    |
| ----- | ------------------ | ------------------------- | ----- |
| 频率    | `frequency`        | 当前输入信号的频率，直接原地修改即可被插件读取   | `f32` |
| 振幅    | `magnitude`        | 当前输入信号的振幅，直接原地修改即可被插件读取   | `f32` |
| 可控制参数 | `a` `b` `c` `d`    | 四个可以被控制的参数，在插件界面的右侧可以直接修改 | `f32` |
| 声道 Id | `sound_channel_id` | 零为左声道，一为右声道，暂时不支持更多声道     | `i32` |
| BPM   | `bpm`              | 当前时间的 bpm，并不一定是恒定的        | `f32` |
| 音轨时间  | `daw_time`         | 当前 daw 播放了多少时间，单位为秒       | `f32` |
| 系统时间  | `sys_time`         | 插件自加载依赖经过了多少时间，单位为秒       | `f32` |
| 窗长    | `window_size`      | FFT 使用的窗口长度               | `i32` |
| 采样率   | `sample_rate`      | 采样率，单位为 Hz                | `f32` |

除非上表说明，否则修改对应的变量不会导致对应参数改变。

你可以**双击** `clear(double click)` / `清空（双击）` 来将控制代码恢复默认，或者点击 `show code` / `展示代码` 来显示当前加载的代码。

在右侧你可以设置四个可控制参数 `a`、`b`、`c`、`d`. 你也可以通过 `out gain` / `输出增益` 来调整输出增益；通过 `window_size` / `FFT 窗长` 设置 FFT 的窗口长度。

插件还提供了两个高级参数，他们会影响窗函数，具体作用见如下公式：

$$
W[i] = F - (1 - F) \cdot \cos\left( \frac{2\pi}{N} [(i + \Delta) \space \text{mod} \space N] \right)
$$

其中 `F` 是 `window factor` / `窗口参数` 而 `Δ` 是 `window offset` / `窗口延迟`，`N` 是 `window_size` / `FFT 窗长`。
## 编译

如果你想直接编译的话大概率会报错，因为我没有使用 github 链接填写 nih_plug 作为依赖，所以你首先需要修改进入 `Cargo.toml` 修改 
`nih-plug` 和 `nih_plug_egui` 的路径。之后你需要参考 [nih_plug](https://github.com/robbert-vdh/nih-plug) 的 `README.MD` 进行编译。

这个插件设置了几个 feature，其默认配置如下

```toml
[features]
default = ["en_us"]
zh_cn_support = []
zh_cn = ["zh_cn_support"]
en_us = []
```

很直白的多语言 feature，不过需要注意的是使用 `zh_cn_support` 时需要字体文件，但是，仓库并没有包含字体文件，所以你需要修改 `lib.rs` 中的 `FONT` 
常量为你自己的字体文件。 `Release` 中我们使用的是 [霞骛新晰黑](https://github.com/lxgw/LxgwNeoXiHei)

开启单个 feature 的话会编译对应语言的版本，如果你开启了两个 feature 则会编译多语言版本。

## 已知问题 & 可能的更新方向

- [ ] 解决脚本语言开销带来的高 CPU 占用
- [ ] 加入映射后频率的可视化
- [ ] 允许脚本存储某些变量以便在不同的 FFT 窗口之间使用

## 协议


GPLv3, 派生自 `nih_plug`.
