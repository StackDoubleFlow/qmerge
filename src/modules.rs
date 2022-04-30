use anyhow::Result;

pub struct CodeGenModule<'src> {
    pub methods: Vec<Option<&'src str>>,
}

pub fn parse(src: &str) -> Result<CodeGenModule> {
    let mut methods = Vec::new();

    if let Some(arr_start) = src.find("static Il2CppMethodPointer s_methodPointers") {
        for line in src[arr_start..].lines().skip(3) {
            if line.starts_with('}') {
                break;
            }
            let name = line.trim().trim_end_matches(',');
            if name == "NULL" {
                methods.push(None);
            } else {
                methods.push(Some(name));
            }
        }
    };

    Ok(CodeGenModule { methods })
}
