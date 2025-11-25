use pyo3::prelude::*;
use crate::net::client::client_connect;
mod net;

#[pyfunction]
async fn connect(url: String) -> PyResult<()> {
    if client_connect(url).await.is_ok() {
        return Ok(())
    }
    Err(pyo3::exceptions::PyRuntimeError::new_err("connection failed"))
}
#[pymodule]
fn data_broker_client(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(connect, module)?)?;
    Ok(())
}