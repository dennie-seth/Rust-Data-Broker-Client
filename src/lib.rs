use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use crate::net::client::client_connect;
mod net;
#[pyfunction]
fn connect(url: String) -> PyResult<()> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

    rt.block_on(async move {
        client_connect(url).await
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("connection failed"))
    })
}

#[pymodule]
fn data_broker_client(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(connect, m)?)?;
    Ok(())
}
