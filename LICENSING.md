# Licensing

The OmniEdge project is transitioning from the GNU General Public License v3 (GPL v3) to a dual-licensing model: **Apache License 2.0** and **MIT License**.

## Rationale for Transition

The shift to Apache 2.0 / MIT dual-licensing for the new Rust-based core and CLI engine is intended to:
1.  **Enhance Permissiveness**: Encourage broader adoption and integration into a wider variety of environments, including those with specific corporate or legal requirements.
2.  **Align with the Ecosystem**: Follow the common practice in the Rust community, where many foundational libraries and projects (including the Rust compiler itself) are dual-licensed this way.
3.  **Encourage Contributions**: Provide a familiar and lightweight legal framework for contributors.

## Licensing Status

-   **New Rust Implementation (`crates/`, `ui/desktop`)**: These components are licensed under the Apache License 2.0 and the MIT License.
-   **Legacy Components (`pkg/`, `cmd/`, `protocol/`, `n2n`)**: The previous Go-based implementation was licensed under GPL v3. These components have been deprecated and removed as part of the migration to the new Rust-based architecture.

## Dual-License Choice

Users and contributors may choose to use the software under the terms of either the Apache License 2.0 or the MIT License.

-   **Apache 2.0**: [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0)
-   **MIT**: [https://opensource.org/licenses/MIT](https://opensource.org/licenses/MIT)
