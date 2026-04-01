use log::{Level, Metadata, Record};

pub fn init() {
    static LOGGER: Logger = Logger;
    log::set_logger(&LOGGER).unwrap();

    // 配置日志框架的最大输出级别：
    // 在内核开发的早期阶段，设置为 Trace，这样就能看到包括底层的每一步调试信息。
    log::set_max_level(log::LevelFilter::Trace);

    info!("Logger Initialized.");
}

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        // 判断当前拦截到的日志级别，是否低于或等于我们全局设定的最大级别
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {   
            // 为不同级别分配终端控制台颜色
            let color_code = match record.level() {
                Level::Error => 31,
                Level::Warn => 33,
                Level::Info => 32,
                Level::Debug => 36,
                Level::Trace => 90,
            };
        
            // 提取产生该日志的代码文件路径和行号
            // 使用 unwrap_or 提供默认值，防止偶尔获取不到信息导致内核 Panic
            let file = record.file_static().unwrap_or("unknown");
            let line = record.line().unwrap_or(0);

            // 组合并输出日志
            // \x1b[{}m  : 告诉终端接下来的字用什么颜色显示
            // \x1b[0m   : 打印完毕后，把终端颜色重置回默认状态，防止后续全变色
            // {:>5}     : 让日志级别（如 INFO, WARN）右对齐，占5个字符宽度，显得整齐美观
            println!(
                "\x1b[{}m[{:>5}] [{}:{}] {}\x1b[0m",
                color_code,
                record.level(),
                file,
                line,
                record.args()
            );
        }
    }

    fn flush(&self) {}
}