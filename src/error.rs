use anyhow::Result;

pub fn exit_on_err<T>(result: Result<T>) -> T {
    // TODO: better error handling
    match result {
        Err(err) => {
            eprintln!("{:?}", err);
            std::process::exit(1);
        }
        Ok(val) => val,
    }
}
