# proxy_2x2 fixture

Synthetic 2×2 adjacent boxes for Phase 6 ProxyBuilder.

Generate with:

```bash
cargo run -p top_rebuild --bin top_rebuild_debug -- proxy \
  --out tests/fixtures/top_rebuild/proxy_2x2/Proxy_L1_0_0.glb \
  --max-triangles 2000 --segments 8
```

This writes child B3DMs under the output sibling `_proxy_children/` and the parent proxy GLB.
Children use world translations at (50,50,10)/(150,50,10)/(50,150,10)/(150,150,10);
parent local origin at (100,100,10).
