use {
    colored::*,
    serde::{Deserialize, Serialize},
    std::{
        cmp::Ordering,
        collections::BTreeMap,
        error::Error,
        ffi::OsString,
        fs::File,
        io::Write,
        path::{Path, PathBuf},
    },
    structopt::StructOpt,
    toml_edit::{Array, Decor, Document, InlineTable, Item, RawString, Table, Value},
};

/// Type alias for shorter return types.
pub type Res<T = ()> = Result<T, Box<dyn Error>>;

pub fn run(mut opt: Opt, config: Config) -> Res {
    let config: ProcessedConfig = config.into();

    if opt.files.is_empty() && opt.folder.is_empty() {
        opt.folder.push(std::env::current_dir()?);
    }

    for folder in opt.folder {
        let files = find_files_recursively(folder, "toml", !opt.silent, &config.excludes);
        opt.files.extend(files);
    }

    for file in opt.files {
        config.process_file(file, opt.check, !opt.silent)?;
    }

    Ok(())
}

/// A TOML entry. Generic to support both `Item` and `Value` entries.
struct Entry<T> {
    key: String,
    value: T,
    decor: Decor,
}

#[derive(StructOpt, Debug, Clone)]
pub struct Opt {
    /// List of .toml files to format.
    /// If no files are provided, and `--scan-folder` is not used then
    /// it will scan for all .toml files in the current directory.
    #[structopt(name = "FILE", parse(from_os_str))]
    pub files: Vec<PathBuf>,

    /// Scan provided folder(s) recursively for .toml files.
    #[structopt(long, parse(from_os_str))]
    pub folder: Vec<PathBuf>,

    /// Only check the formatting, returns an error if the file is not formatted.
    /// If not provide the files will be overritten.
    #[structopt(short, long)]
    pub check: bool,

    /// Disables verbose messages.
    #[structopt(short, long)]
    pub silent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GenericConfig<Keys> {
    /// Important keys in non-inline tables.
    /// Will be sorted first, then any non-important keys will be
    /// sorted lexicographically.
    #[serde(default)]
    pub keys: Keys,

    /// Important keys in inline tables.
    /// Will be sorted first, then any non-important keys will be
    /// sorted lexicographically.
    #[serde(default)]
    pub inline_keys: Keys,

    /// Does it sort arrays?
    /// In case of mixed types, string will be ordered first, then
    /// other values in original order.
    #[serde(default)]
    pub sort_arrays: bool,

    #[serde(default)]
    /// Paths to ignore when scanning directories.
    pub excludes: Vec<String>,
}

pub type Config = GenericConfig<Vec<String>>;
pub type ProcessedConfig = GenericConfig<BTreeMap<String, usize>>;

const CONFIG_FILE: &str = "toml-maid.toml";

impl Config {
    pub fn read_from_file() -> Option<Config> {
        let mut path: PathBuf = std::env::current_dir().ok()?;
        let filename = Path::new(CONFIG_FILE);

        loop {
            path.push(filename);

            if path.is_file() {
                let text = std::fs::read_to_string(&path).ok()?;
                let config: Self = toml::from_str(&text).ok()?;
                return Some(config);
            }

            if !(path.pop() && path.pop()) {
                // remove file && remove parent
                return None;
            }
        }
    }
}

impl From<Config> for ProcessedConfig {
    fn from(x: Config) -> Self {
        let mut res = Self {
            keys: BTreeMap::new(),
            inline_keys: BTreeMap::new(),
            sort_arrays: x.sort_arrays,
            excludes: x.excludes,
        };

        for (i, key) in x.keys.iter().enumerate() {
            res.keys.insert(key.clone(), i);
        }

        for (i, key) in x.inline_keys.iter().enumerate() {
            res.inline_keys.insert(key.clone(), i);
        }

        res
    }
}

fn absolute_path(path: impl AsRef<Path>) -> Res<String> {
    Ok(std::fs::canonicalize(&path)?.to_string_lossy().to_string())
}

pub fn find_files_recursively(
    dir_path: impl AsRef<Path>,
    extension: &str,
    verbose: bool,
    excludes: &[String],
) -> Vec<PathBuf> {
    macro_rules! continue_on_err {
        ($in:expr, $context:expr) => {
            match $in {
                Ok(e) => e,
                Err(e) => {
                    if verbose {
                        eprintln!("Error while {}: {}", $context, e);
                    }
                    continue;
                }
            }
        };
    }

    let dir_path: PathBuf = dir_path.as_ref().to_owned();
    let mut matches = vec![];
    let extension: OsString = extension.into();
    let config_file: OsString = CONFIG_FILE.into();

    let excludes: Vec<_> = excludes
        .iter()
        .map(|v| glob::Pattern::new(&v).expect("invalid pattern in 'excludes'"))
        .collect();

    for entry in ignore::WalkBuilder::new(&dir_path)
        .skip_stdout(true)
        .filter_entry(move |entry| {
            let path = entry.path();
            let relative_path = path
                .strip_prefix(&dir_path)
                .expect("scanned file should be inside scanned dir");

            for exclude in &excludes {
                if exclude.matches_path(&relative_path) {
                    return false;
                }
            }

            true
        })
        .build()
    {
        let entry = continue_on_err!(entry, "getting file info");
        let file_type =
            continue_on_err!(entry.file_type().ok_or("no file type"), "getting file type");
        let path = entry.path().to_owned();

        // We ignore directories, as `ignore::Walk` performs the recursive search.
        if file_type.is_dir() {
            continue;
        }

        // We ignore non .toml files
        if path.extension() != Some(&extension) {
            continue;
        }

        // We don't format `toml-maid.toml` files as the order is important.
        // TODO: Still format but override `sort_arrays`.
        if path.file_name() == Some(&config_file) {
            continue;
        }

        matches.push(path);
    }

    matches
}

impl ProcessedConfig {
    /// Process the provided file.
    pub fn process_file(&self, path: impl AsRef<Path>, check: bool, verbose: bool) -> Res<()> {
        let absolute_path = absolute_path(&path)?;
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            eprintln!(
                "Error while reading file \"{}\" : {}",
                absolute_path,
                e.to_string().red()
            );
            std::process::exit(3);
        });

        let doc = text.parse::<Document>()?;
        let trailing = doc.trailing().as_some_str().trim_end();

        let output_table = self.format_table(&doc)?;
        let mut output_doc: Document = output_table.into();
        output_doc.set_trailing(trailing); // Insert back trailing content (comments).
        let output_text = format!("{}\n", output_doc.to_string().trim());

        if check {
            if text != output_text {
                eprintln!("Check fails : {}", absolute_path.red());
                std::process::exit(2);
            } else if verbose {
                println!("Check succeed: {}", absolute_path.green());
            }
        } else if text != output_text {
            let mut file = File::create(&path)?;
            file.write_all(output_text.as_bytes())?;
            file.flush()?;
            if verbose {
                println!("Overwritten: {}", absolute_path.blue());
            }
        } else if verbose {
            println!("Unchanged: {}", absolute_path.green());
        }

        Ok(())
    }

    /// Format a `Table`.
    /// Consider empty lines as "sections" and will not sort accross sections.
    /// Comments at the start of the section will stay at the start, while
    /// comments attached to any other line will stay attached to that line.
    fn format_table(&self, table: &Table) -> Res<Table> {
        let mut formated_table = Table::new();
        formated_table.set_implicit(true); // avoid empty `[dotted.keys]`
        let prefix = table
            .decor()
            .prefix()
            .map(|s| s.as_some_str())
            .unwrap_or("");
        let suffix = table
            .decor()
            .suffix()
            .map(|s| s.as_some_str())
            .unwrap_or("");
        formated_table.decor_mut().set_prefix(prefix);
        formated_table.decor_mut().set_suffix(suffix);

        let mut section_decor = Decor::default();
        let mut section = Vec::<Entry<Item>>::new();

        let sort = |x: &Entry<Item>, y: &Entry<Item>| {
            let xord = self.keys.get(&x.key);
            let yord = self.keys.get(&y.key);

            match (xord, yord) {
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (Some(x), Some(y)) => x.cmp(y),
                (None, None) => x.key.cmp(&y.key),
            }
        };

        // Iterate over all original entries.
        for (i, (key, item)) in table.iter().enumerate() {
            let mut key_decor = table.key_decor(key).unwrap().clone();

            // First entry can be decored (prefix).
            // In that case we want to keep that decoration at the start of the section.
            if i == 0 {
                if let Some(prefix) = key_decor.prefix() {
                    let prefix = prefix.as_some_str();
                    if !prefix.is_empty() {
                        section_decor.set_prefix(prefix);
                        key_decor.set_prefix("".to_string());
                    }
                }
            }
            // Later entries can contain a new-line prefix decor.
            // It means it is a new section, and sorting must not cross
            // section boundaries.
            else if let Some(prefix) = key_decor.prefix() {
                let prefix = prefix.as_some_str();
                if prefix.starts_with('\n') {
                    // Sort keys and insert them.
                    section.sort_by(sort);

                    for (i, mut entry) in section.into_iter().enumerate() {
                        // Add section prefix.
                        if i == 0 {
                            if let Some(prefix) = section_decor.prefix() {
                                let prefix = prefix.as_some_str();
                                entry.decor.set_prefix(prefix);
                            }
                        }

                        formated_table.insert(&entry.key, entry.value);
                        *formated_table.key_decor_mut(&entry.key).unwrap() = entry.decor;
                    }

                    // Cleanup for next sections.
                    section = Vec::new();
                    section_decor = Decor::default();
                    section_decor.set_prefix(prefix);
                    key_decor.set_prefix("".to_string());
                }
            }

            // Remove any trailing newline in decor suffix.
            if let Some(suffix) = key_decor.suffix().map(|x| x.to_owned()) {
                let suffix = suffix.as_some_str();
                key_decor.set_suffix(suffix.trim_end_matches('\n'));
            }

            // Format inner item.
            let new_item = match item {
                Item::None => Item::None,
                Item::Value(inner) => Item::Value(self.format_value(inner, false)?),
                Item::Table(inner) => Item::Table(self.format_table(inner)?),
                // TODO : Doesn't seem we have any of those.
                Item::ArrayOfTables(inner) => Item::ArrayOfTables(inner.clone()),
            };

            section.push(Entry {
                key: key.to_string(),
                value: new_item,
                decor: key_decor,
            });
        }

        // End of entries, we insert remaining section.
        section.sort_by(sort);

        for (i, mut entry) in section.into_iter().enumerate() {
            // Add section prefix.
            if i == 0 {
                if let Some(prefix) = section_decor.prefix() {
                    let prefix = prefix.as_some_str();
                    entry.decor.set_prefix(prefix);
                }
            }

            formated_table.insert(&entry.key, entry.value);
            *formated_table.key_decor_mut(&entry.key).unwrap() = entry.decor;
        }

        Ok(formated_table)
    }

    /// Format inline tables `{ key = value, key = value }`.
    /// TOML doesn't seem to support inline comments, so we just override entries decors
    /// to respect proper spaces.
    pub fn format_inline_table(&self, table: &InlineTable, last: bool) -> Res<InlineTable> {
        let mut formated_table = InlineTable::new();
        if last {
            formated_table.decor_mut().set_suffix(" ");
        }

        let mut entries = Vec::<Entry<Value>>::new();

        let sort = |x: &Entry<Value>, y: &Entry<Value>| {
            let xord = self.inline_keys.get(&x.key);
            let yord = self.inline_keys.get(&y.key);

            match (xord, yord) {
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (Some(x), Some(y)) => x.cmp(y),
                (None, None) => x.key.cmp(&y.key),
            }
        };

        for (key, value) in table.iter() {
            let mut key_decor = table.key_decor(key).unwrap().clone();

            // Trim decor.
            key_decor.set_prefix(" ");
            key_decor.set_suffix(" ");

            let new_value = value.clone();

            entries.push(Entry {
                key: key.to_string(),
                value: new_value,
                decor: key_decor,
            });
        }

        entries.sort_by(sort);

        let len = entries.len();
        for (i, entry) in entries.into_iter().enumerate() {
            let new_value = self.format_value(&entry.value, i + 1 == len)?;

            formated_table.insert(&entry.key, new_value);
            *formated_table.key_decor_mut(&entry.key).unwrap() = entry.decor;
        }

        Ok(formated_table)
    }

    /// Format a `Value`.
    pub fn format_value(&self, value: &Value, last: bool) -> Res<Value> {
        Ok(match value {
            Value::Array(inner) => Value::Array(self.format_array(inner, last)?),
            Value::InlineTable(inner) => Value::InlineTable(self.format_inline_table(inner, last)?),
            v => {
                let mut v = v.clone();

                // Keep existing prefix/suffix with correct format.
                let prefix = v
                    .decor()
                    .prefix()
                    .map(|x| x.as_some_str().trim())
                    .unwrap_or("");

                let prefix = if prefix.is_empty() {
                    prefix.to_string()
                } else {
                    format!(" {}", prefix)
                };

                let suffix = v
                    .decor()
                    .suffix()
                    .map(|x| x.as_some_str().trim())
                    .unwrap_or("");

                let suffix = if suffix.is_empty() {
                    suffix.to_string()
                } else {
                    format!(" {}", suffix)
                };

                // Convert simple '...' to "..."
                // Doesn't modify strings starting with multiple ' as they
                // could be multiline literals.
                // Doesn't modify strings containing \ or "
                let mut display = v.clone().decorated("", "").to_string();
                if display.starts_with('\'')
                    && !display.starts_with("''")
                    && display.find(&['\\', '"'][..]).is_none()
                {
                    if let Some(s) = display.strip_prefix('\'') {
                        display = s.to_string();
                    }

                    if let Some(s) = display.strip_suffix('\'') {
                        display = s.to_string();
                    }

                    v = display.into();
                }

                // Handle surrounding spaces.
                if last {
                    v.decorated(&format!("{} ", prefix), &format!("{} ", suffix))
                } else {
                    v.decorated(&format!("{} ", prefix), &suffix.to_string())
                }
            }
        })
    }

    /// Format an `Array`.
    /// Detect if the array is inline or multi-line, and format it accordingly.
    /// Support comments in multi-line arrays.
    /// With config `sort_string_arrays` the array String entries will be sorted, otherwise will be kept
    /// as is.
    fn format_array(&self, array: &Array, last: bool) -> Res<Array> {
        let mut values: Vec<_> = array.iter().cloned().collect();

        if self.sort_arrays {
            values.sort_by(|x, y| match (x, y) {
                (Value::String(x), Value::String(y)) => x.value().cmp(y.value()),
                (Value::String(_), _) => Ordering::Less,
                (_, Value::String(_)) => Ordering::Greater,
                (_, _) => Ordering::Equal,
            });
        }

        let mut new_array = Array::new();

        for value in values.into_iter() {
            new_array.push_formatted(value);
        }

        let mut multiline = array.trailing().as_some_str().starts_with('\n');
        if !multiline {
            for item in array.iter() {
                if let Some(prefix) = item.decor().prefix() {
                    if prefix.as_some_str().contains('\n') {
                        multiline = true;
                        break;
                    }
                }

                if let Some(suffix) = item.decor().suffix() {
                    if suffix.as_some_str().contains('\n') {
                        multiline = true;
                        break;
                    }
                }
            }
        }

        // Multiline array
        if multiline {
            let mut trailing = format!(
                "{}\n",
                array
                    .trailing()
                    .as_some_str()
                    .trim_matches(&[' ', '\t'][..])
                    .trim_end()
            );

            if !trailing.starts_with('\n') {
                trailing = format!(" {trailing}");
            }

            new_array.set_trailing(&trailing);
            new_array.set_trailing_comma(true);

            for value in new_array.iter_mut() {
                let prefix = value
                    .decor()
                    .prefix()
                    .map(|s| s.as_some_str())
                    .unwrap_or("")
                    .trim_matches(&[' ', '\t'][..])
                    .trim_end_matches('\n');

                let mut prefix = if !prefix.is_empty() {
                    format!("{}\n\t", prefix)
                } else {
                    "\n\t".to_string()
                };

                if !prefix.starts_with('\n') {
                    prefix = format!(" {prefix}");
                }

                let mut suffix = value
                    .decor()
                    .suffix()
                    .map(|s| s.as_some_str())
                    .unwrap_or("")
                    .trim_matches(&[' ', '\t', '\n'][..])
                    .to_string();

                // If the suffix is non-empty it is important to add a trailing
                // new line so that the comma is not part of a comment
                if !suffix.is_empty() {
                    suffix.push('\n');
                }

                let formatted_value = self.format_value(value, false)?;
                *value = formatted_value.decorated(&prefix, &suffix);
            }
        }
        // Inline array
        else {
            new_array.set_trailing("");
            new_array.set_trailing_comma(false);

            let len = new_array.len();
            for (i, value) in new_array.iter_mut().enumerate() {
                *value = self.format_value(value, i + 1 == len)?;
            }
        }

        new_array.decor_mut().set_prefix(" ");
        new_array
            .decor_mut()
            .set_suffix(if last { " " } else { "" });

        Ok(new_array)
    }
}

trait RawStringExt {
    fn as_some_str(&self) -> &str;
}

impl RawStringExt for RawString {
    fn as_some_str(&self) -> &str {
        self.as_str().expect(
            "parsed documents should not be spanned, which means `as_str` should \
            always return Some(_)",
        )
    }
}
