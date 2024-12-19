use std::env;

pub fn debug(msg: String) {
    let args: Vec<String> = env::args().collect();

    if args.len() >= 3 {
        let arg = match args.get(1) {
            Some(cmd) => cmd,
            None => {
                eprintln!("Unknown arguments provided");
                return;
            }
        };

        let dcmd = String::from("-d");
        if arg == dcmd.as_str() {
            let dval = match args.get(2) {
                Some(val) => val,
                None => {
                    eprintln!("Unknown arguments provided!");
                    return;
                }
            };

            let gdbg = match dval.parse::<u64>() {
                Ok(val) => val,
                Err(e) => {
                    eprintln!("Unable to parse number from argument: {}", e);
                    return;
                }
            };

            if gdbg > 0 {
                println!("{}", msg);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
