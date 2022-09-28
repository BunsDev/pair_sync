This is just temp notes


Potentially change generic trait definitions from:

```rust

    pub async fn get_price_a_per_b<P>(&self, provider: Arc<Provider<P>>)
    where
        P: JsonRpcClient,
    {}
```

to:

```rust
    pub async fn get_price_a_per_b<P: JsonRpcClient>(&self, provider: Arc<Provider<P>>)
    {}
```