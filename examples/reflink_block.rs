use std::fs::File;
use std::num::NonZeroU64;

// cargo run --example reflink_block V:/file.bin V:/file_cow.bin 4096

fn main() -> std::io::Result<()> {
    let args: Vec<_> = std::env::args().collect();

    let [_, src_file, tgt_file, cluster_size] = &args[..] else {
        eprintln!(
            "Usage: {} <source_file> <target_file> <cluster_size>",
            args[0]
        );
        return Ok(());
    };
    let cluster_size: NonZeroU64 = cluster_size.parse().expect("cannot parse cluster size");

    let from_file = File::open(src_file)?;
    let len = from_file.metadata()?.len();
    let to_file = File::create(tgt_file)?;
    to_file.set_len(len)?;

    let mut offset = 0u64;
    while offset < len as u64 {
        println!("reflink {offset}, {cluster_size}");
        reflink_copy::ReflinkBlockBuilder::new(&from_file, &to_file, cluster_size)
            .from_offset(offset)
            .to_offset(offset)
            .cluster_size(cluster_size)
            .reflink_block()?;

        offset += cluster_size.get();
    }
    Ok(())
}
