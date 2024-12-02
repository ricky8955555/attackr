# attackr

## 简介

attackr 是使用 [Rust](https://www.rust-lang.org) + [Rocket](https://rocket.rs) + [MiniJinja](https://github.com/mitsuhiko/minijinja) 开发的 CTF 平台，并使用 [Koto](https://koto.dev/) 脚本语言实现动态积分及对用户倍数赋分支持以及事件推送支持。

由于设计限制，本平台只支持在单个实例上配置单个比赛，且不支持配置多实例进行负载平衡，适合小型比赛使用。

## 局限性

- 仅支持在单个实例进行单个比赛
- 使用 SQLite 数据库，不适合大并发场景
- 只支持控制本机 Docker，无法实现多设备 Docker 负载平衡
- 不支持二进制产物及附件的内容分发
- 使用 SSR (服务端渲染)，对服务器性能有一定的消耗
- 不支持数据渲染分页面，在访问具有大量数据的页面下可能会导致浏览器卡顿

## 功能

### 核心功能

- 主页
    - 可配置并显示比赛名称、介绍及时间 (支持调节时区)
    - 显示当前比赛状态
- 用户
    - 登录 / 注册用户
    - 使用 Argon2id 对密码进行 Hash 储存
    - 查看用户主页
    - 修改用户信息
    - 支持通过 Gravatar 获取用户头像
    - 可在用户主页查看各题目的解题时间及得分情况
    - 可在用户主页查看各题集的解题及得分情况
    - 支持昵称中使用 Tag 并根据 Hash 设置 Tag 颜色
    - 限制被禁用的用户进行登录及访问其他页面
    - 可配置用户注册审核
    - 重置密码时注销所有会话
    - 可配置 Session 有效时间
- 题目
    - 静态题目 (Flag 在题目创建时确定) 
    - 动态题目 (Flag 在用户触发构建时确定)
    - Docker 镜像的构建及容器的启停
    - 二进制产物构建及下载
    - 重新构建动态题目
    - 支持 Markdown 题目描述
    - 支持区分题集 (可用于实现区分题目方向)
    - 支持区分难度 (可自定义难度的颜色)
    - 可配置脚本实现动态积分
    - 可配置脚本实现单个题目对指定排名的用户进行倍数赋分 (可用于实现前三血功能)
    - 支持单个题目多个产物 (包括二进制产物和 Docker 产物)
    - 显示题目当前通过人数及分数
    - 禁止用户在比赛前访问题目
    - 检验用户输入 Flag
    - 可配置题目及产物的储存路径
    - 可配置 Docker 监听的地址及端口 (支持 IPv4、IPv6)
    - 可配置 Docker 端口映射 (仅作为对用户的显示，并不能实现功能上的映射)
    - 可配置 Docker 容器自动销毁时间
    - 可限制 Docker 的 CPU、内存、储存占用
    - 可通过 Bind 挂载的 `/var/lib/attackr` 公开 Docker 容器相关状态文件 (可用于实现前置认证)
    - 可配置在题目解出后自动清理产物
    - 可配置题目是否公开
    - 可配置是否显示未分类题集的题目
- 榜单
    - 积分变化曲线 (可显示得分下降)
    - 用户排名及各题目得分表
    - 可显示各题集的分榜单
    - 禁止用户在比赛前访问榜单

### 管理员功能

- 用户
    - 修改用户信息 (包括权限组，在修改用户至管理员后将取消所有得分)
    - 注销用户所有会话
    - 启用 / 禁用用户 (禁用用户后用户将无法进行任何操作，并将取消所有得分)
    - 在去除取消得分的操作后可恢复用户得分
    - 按照启用 / 禁用情况区分用户
- 题目
    - 添加题目
    - 修改题目信息 (不支持修改题目源码及附件)
    - 查看题目详情 (包括构建脚本参数)
    - 批量公开题目
    - 重新计算题目分数及用户得分
- 产物
    - 查看产物信息
    - 删除用户产物
    - 重新构建产物
- 题集 (题目类别)
    - 添加 / 修改 / 删除题集
- 难度
    - 添加 / 修改 / 删除难度
- 提交记录
    - 查看用户提交记录
    - 可筛选指定用户 / 题目查看提交记录

### 事件推送

> 该功能需通过配置条件编译 (Features) 启用，详见 [构建](#构建) 一节。

利用 Koto 脚本实现对事件 (Activity) 的监听。

- 题目
    - 解题通过事件 (Solved)

## 截图

<details>

<summary>显示所有截图</summary>

![](assets/1.webp)

![](assets/2.webp)

![](assets/4.webp)

![](assets/3.webp)

</details>

## 构建

目前支持的条件编译 (Features):

- `koto_exec`: 在 Koto 脚本中添加 `exec` 以支持外部程序的运行
- `koto_json`: 在 Koto 脚本中添加 [`json`](https://github.com/koto-lang/koto/tree/main/libs/json) 库依赖
- `koto_tempfile`: 在 Koto 脚本中添加 [`tempfile`](https://github.com/koto-lang/koto/tree/main/libs/tempfile) 库依赖
- `koto_random`: 在 Koto 脚本中添加 [`random`](https://github.com/koto-lang/koto/tree/main/libs/random) 库依赖
- `activity`: 启用事件推送 (Activity) 支持

选择相应的所需支持，使用指令 `./build.sh [features]`，将 `[features]` 替换为所需的编译条件，多个编译条件使用 `,` 分隔开。如无需启用其他支持，置空 `[features]` 即可。

示例:

- 不带其他支持: `./build.sh`
- 带 `koto_exec` 及 `activity` 支持: `./build.sh koto_exec,activity`

编译成功后将会在根目录下产生 `attackr.tar.gz`，将内容解压至工作目录即可。

## 配置及脚本编写

### 平台配置

参见 [examples/configs](examples/configs) 中给出的示例。

### 动态积分脚本

动态积分脚本必须要提供两个函数:

- `calculate_points`: 计算题目动态积分
- `calculate_factor`: 计算题目对指定排名用户的分数倍数

脚本可放在任意位置，并将脚本路径配置到 `challenge.yml` 配置文件下的 `dynpoints` 配置项 (详见 [examples/configs/challenge.yml](examples/configs/challenge.yml))

详细参见 [examples/dynpoints](examples/dynpoints) 中给出的示例。

## 题目源代码编写

> 注: 如果不需要使用平台进行构建的，可以先在本地构建成产物，然后再通过附件形式上传到平台，可减小平台相应构建负担。

题目源代码需要以 *Tarball Identity* (`.tar`) 档案包的形式上传到平台，并且档案包根目录下必须含有 `build.yml` 文件指引源代码的构建。

详细参见 [examples/challenges](examples/challenges) 中给出的示例。

## 事件监听脚本编写

配置文件参见 [examples/configs/activity.yml](examples/configs/activity.yml) 中给出的示例。

事件监听脚本参见 [examples/activity](examples/activity) 中给出的示例。

## 运行

在编写好相应配置文件及脚本之后直接运行 `attackr` 即可。
