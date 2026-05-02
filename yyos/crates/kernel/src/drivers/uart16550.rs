use core::fmt;
use x86_64::instructions::port::Port;

/// A port-mapped UART 16550 serial interface.
pub struct SerialPort {
    port: u16,
}

impl SerialPort {
    pub const fn new(port: u16) -> Self {
        Self { port }
    }

    /// Initializes the serial port.
    pub fn init(&self) {
        let mut line_control_port = Port::new(self.port + 3);
        let mut fifo_control_port = Port::new(self.port + 2);
        let mut interrupt_enable_port = Port::new(self.port + 1);
        let mut modem_control_port = Port::new(self.port + 4);
        let mut ier_port: Port<u8> = Port::new(0x3F8 + 1);

        unsafe {
            // 1. 启用中断
            interrupt_enable_port.write(0x01u8);

            // 2. 设置波特率 (115200)
            // 开启 DLAB (Divisor Latch Access Bit)
            line_control_port.write(0x80u8);
            // 设置分频器 (115200 波特率的分频值为 1)
            Port::<u8>::new(self.port).write(0x01u8);      // 低 8 位
            Port::<u8>::new(self.port + 1).write(0x00u8);  // 高 8 位

            // 3. 设置数据格式: 8 数据位, 无校验, 1 停止位
            line_control_port.write(0x03u8);

            // 4. 启用并重置 FIFO 缓冲区
            fifo_control_port.write(0xC7u8);

            // 5. 设置调制解调器控制位 (DTR, RTS, Out2)
            modem_control_port.write(0x0Bu8);

            ier_port.write(0x01); // 开启 Data Available Interrupt
        }
    }

    /// Sends a byte on the serial port.
    pub fn send(&mut self, data: u8) {
        let mut status_port = Port::<u8>::new(self.port + 5);
        let mut data_port = Port::<u8>::new(self.port);

        unsafe {
            // 关键：轮询检查 Line Status Register (LSR)
            // 检查第 5 位 (0x20) 是否为 1 (Transmitter Holding Register Empty)
            // 只有当缓冲区为空时，才能安全地写入下一个字节
            while (status_port.read() & 0x20) == 0 {
                core::hint::spin_loop();
            }
            // 向数据寄存器写入字节
            data_port.write(data);
        }
    }

    /// Receives a byte on the serial port no wait.
    pub fn receive(&mut self) -> Option<u8> {
        let mut status_port = Port::<u8>::new(self.port + 5);
        let mut data_port = Port::<u8>::new(self.port);

        unsafe {
            // 检查第 0 位是否为 1 (Data Ready)
            if (status_port.read() & 1) == 0 {
                None
            } else {
                Some(data_port.read())
            }
        }
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            // 针对换行符自动补全回车符 \r，以确保终端显示正常
            if byte == b'\n' {
                self.send(b'\r');
            }
            self.send(byte);
        }
        Ok(())
    }
}

