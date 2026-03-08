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