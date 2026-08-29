# <Client> + MemoryWhale

Use this structure for every client integration. Keep these eight headings,
in this order. Remove placeholder text. Write "Not applicable" plus a one-line
reason when the client does not support a section.

Client-specific details belong under these headings as subsections. Do not
add extra required top-level headings. Do not copy setup from another guide;
verify commands and paths against the client's current documentation.

## Status

State whether the configuration is verified and against which client version
or documentation date. Link the authoritative pages you used.

## Requirements

List MemoryWhale, client, PATH, platform, and feature requirements.

## Setup

Show the exact configuration path and the minimal configuration. Link the
authoritative client documentation used to verify it.

## Verify

Give a deterministic transport or tool-discovery check, plus any client UI
or CLI command that proves the server is connected.

## Available capabilities

| Capability | Available |
| --- | --- |
| MCP memory access | Yes / No / Unverified |
| Automatic execution capture | Yes / No / Unverified |
| Memory-use guidance | Yes / No / Unverified |

Declare only what this repository demonstrates. MCP access is not automatic
execution capture. List the MemoryWhale tools the client actually exposes, or
state that tool discovery was not verified.

## Example prompt

> Use MemoryWhale to check whether I encountered a similar failure before.

Replace this with a client-specific prompt when the client needs different
wording. If the client is not an agent, write “Not applicable” and say why.

## Troubleshooting

Cover PATH, restart, configuration location, and data-directory issues.

## Uninstall

Explain how to remove client configuration without deleting MemoryWhale data.

## Contribution checklist

- [ ] Headings match this template, in this order.
- [ ] Setup and Verify were checked against current client docs, not another
      MemoryWhale guide.
- [ ] Capabilities match repository evidence.
- [ ] Automatic capture is claimed only when a verified hook or equivalent
      exists.
- [ ] Uninstall does not delete the MemoryWhale database.
- [ ] The client is listed in [README.md](README.md).
