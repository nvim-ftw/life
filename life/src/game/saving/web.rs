use super::DataStorage;
use web_sys::Storage;

pub struct WebStorage<'a, T>
where
    T: serde::Serialize + for<'b> serde::Deserialize<'b> + Default
{
    storage: Storage,
    data: T,
    key: &'a str,
}

impl<T> DataStorage for WebStorage<'_, T>
where
    T: serde::Serialize + for<'a> serde::Deserialize<'a> + Default + Clone
{
    type Data = T;
    type Error = anyhow::Error;
    fn new(identifier: &str) -> Result<(WebStorage<T>, T), anyhow::Error> {
        let storage = web_sys::window()?.local_storage()??;
        let existing_data = storage.get_item(identifier)?
            .and_then(|s| {
                serde_json::from_str(&s).ok().map(|v| (v, s))
            })
            .unwrap_or((T::default(), serde_json::to_string_pretty(&T::default())?));
        storage.set_item(identifier, &existing_data.1)?;

        Ok((WebStorage {
            storage,
            data: existing_data.0.clone(),
            key: identifier,
        }, existing_data.0))
    }
    fn get(&self) -> &T {
        &self.data
    }
    fn set(&mut self, data: T) {
        self.data = data;
    }
    fn finish(&mut self) -> Result<(), anyhow::Error> {
        let json_str = serde_json::to_string_pretty(&self.data)?;
        self.storage.set_item(self.key, json_str)?;
        Ok(())
    }
}
