use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use pyo3_asyncio::tokio::init_multi_thread_once;
use crate::net::client::{client_connect, client_send, PyBrokerClient};
mod net;

#[pyfunction]
unsafe fn connect(py: Python<'_>, url: String) -> PyResult<&PyAny> {
    pyo3_asyncio::tokio::future_into_py(py, async move {
        match client_connect(url).await {
            Ok(client) => {
                Python::with_gil(|py| {
                    let py_client = PyBrokerClient {
                        client: client.clone(),
                    };
                    Py::new(py, py_client).map_err(Into::into)
                })
            }
            Err(_) => Err(pyo3::exceptions::PyRuntimeError::new_err("connection failed")),
        }
    })
}
#[pyfunction]
unsafe fn send<'py>(py: Python<'py>, client: &PyBrokerClient, path: String) -> PyResult<&'py PyAny> {
    let client = client.client.clone();
    pyo3_asyncio::tokio::future_into_py(py, async move {
        match client_send(client, &path).await {
            Ok(_) => Ok(()),
            Err(err) => Err(pyo3::exceptions::PyRuntimeError::new_err(format!("failed to send, {err}"))),
        }
    })
}
#[pymodule]
fn data_broker_client(py: Python<'_>, m: &PyModule) -> PyResult<()> {
    pyo3_asyncio::tokio::init_multi_thread_once();
    m.add_function(wrap_pyfunction!(connect, m)?)?;
    m.add_function(wrap_pyfunction!(send, m)?)?;
    Ok(())
}
