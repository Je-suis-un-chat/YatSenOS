pub fn humanized_size(mut size: u64) -> (f64, &'static str)
{
    let units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let mut size_f = size as f64;
    let mut index = 0;

    while size_f >= 1024.0 && index < units.len() -1
    {
        size_f /= 1024.0;
        index += 1;
    }

    (size_f, units[index])
}
#[cfg(test)]  //用于条件编译，只有在cargo test 运行测试时，才编译下面的模块。
mod tests //内部子模块
{
    use super::*;  //把父模块里的所有内容都拉取到测试模块里
    //use关键字用来将其他作用域的项引入当前作用域
    //super代表当前模块的上一级
    //* 是通配符

    #[test] //标记下面的函数是测试函数，测试运行器会自动找到所有带这个标记的函数并执行
    fn test_humanized_size()
    {
        let byte_size = 1554056;
        let (size, unit) = humanized_size(byte_size);

        assert_eq!("Size : 1.4821 MiB", format!("Size : {:.4} {}",size,unit));
        //断言两个值相等的宏
    }
}