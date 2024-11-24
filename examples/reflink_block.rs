use std::fs::File;

fn main() -> std::io::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!(
            "Usage: {} <source_file> <target_file> <cluster_size>",
            args[0]
        );
        return Ok(());
    }
    let src_file = &args[1];
    let tgt_file = &args[2];
    let cluster_size: u64 = args[3].parse().expect("cannot parse cluster size");
    let from_file = File::open(src_file)?;
    let len = from_file.metadata()?.len();
    let to_file = File::create(tgt_file)?;
    to_file.set_len(len)?;
    let mut offset = 0u64;
    while offset < len as u64 {
        println!("reflink {offset}, {cluster_size}");
        reflink_copy::reflink_block(&from_file, offset, &to_file, offset, cluster_size)?;
        offset += cluster_size;
    }
    Ok(())
}
