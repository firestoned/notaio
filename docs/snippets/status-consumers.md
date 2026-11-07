!!! info "Partly implemented"
    notaio's side of this page is built and released: it validates, signs and publishes every
    bundle, and ships the verifier consumers link. Of the consumers, **mediatore reads the bundles
    in shadow**: it verifies them, keeps their version floor, and reports every way its own
    configuration allows more than the bundle, refusing nothing yet. **mediatore enforcing the
    bundle**, **the guest verifying it and rendering the jail from it**, and **the gateway's
    allowlist** are planned (roadmap 00, phases 2 to 4). Where this page says a consumer enforces,
    read "will enforce".
