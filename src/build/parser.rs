pub fn is_struct_name(name: &str) -> bool {
    let len = name.len();
    name.len() > 42
        && &name[len - 42..len - 40] == "_t"
        && name[len - 40..]
            .chars()
            .all(|c| ('A'..='Z').contains(&c) || ('0'..='9').contains(&c))
}

pub struct FnDecl<'a> {
    pub return_ty: &'a str,
    pub name: &'a str,
    pub params: &'a str,
    pub inline: bool,
}

impl<'a> FnDecl<'a> {
    pub fn try_parse(line: &'a str) -> Option<Self> {
        let (line, inline) = if let Some(line) =
            line.strip_prefix("IL2CPP_EXTERN_C IL2CPP_METHOD_ATTR")
        {
            (line, false)
        } else if let Some(line) = line.strip_prefix("IL2CPP_EXTERN_C inline IL2CPP_METHOD_ATTR") {
            (line, true)
        } else {
            (
                line.strip_prefix("IL2CPP_EXTERN_C inline  IL2CPP_METHOD_ATTR")?,
                true,
            )
        };

        let param_start = line.find('(')?;
        let params = &line[param_start..];
        let rest = line[..param_start].trim();

        let name = rest.split_whitespace().last()?;
        let return_ty = rest[..rest.len() - name.len()].trim();

        Some(FnDecl {
            return_ty,
            name,
            params,
            inline,
        })
    }
}

pub fn try_parse_call(line: &str, include_inline: bool) -> Option<&str> {
    let possible_name = if let Some(pos) = line.find("= ") {
        &line[pos + 2..]
    } else {
        line.trim()
    };
    let possible_name = possible_name.split('(').next().unwrap();
    if possible_name.ends_with("_inline") && !include_inline {
        // Inlined functions will be defined in the same file anyways
        return None;
    }
    let len = possible_name.len();
    if possible_name.len() <= 42 {
        return None;
    }
    if &possible_name[len - 42..len - 40] == "_m" {
        let valid_id = possible_name[possible_name.len() - 40..]
            .chars()
            .all(|c| ('A'..='Z').contains(&c) || ('0'..='9').contains(&c));
        if valid_id {
            return Some(possible_name);
        }
    }

    None
}
