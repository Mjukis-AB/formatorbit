# Formatorbit Plugin System

Formatorbit supports Python plugins that extend its functionality with custom decoders, traits, expression functions, and more.

## Table of Contents

- [Quick Start](#quick-start)
- [Building with Plugin Support](#building-with-plugin-support)
- [Plugin Directory](#plugin-directory)
- [Plugin Types](#plugin-types)
  - [Decoders](#decoders)
  - [Expression Variables](#expression-variables)
  - [Expression Functions](#expression-functions)
  - [Traits](#traits)
  - [Visualizers](#visualizers)
  - [Currencies](#currencies)
- [Plugin Structure](#plugin-structure)
- [CLI Commands](#cli-commands)
- [Configuration](#configuration)
- [Sample Plugins](#sample-plugins)
- [API Reference](#api-reference)
- [Troubleshooting](#troubleshooting)

## Quick Start

1. **Build with plugin support:**
   ```bash
   cargo build -p formatorbit-cli --features plugins --release
   ```

2. **Enable sample plugins:**
   ```bash
   # Create plugin directory
   mkdir -p ~/.config/forb/plugins

   # Copy and enable sample plugins
   cp sample-plugins/math_ext.py.sample ~/.config/forb/plugins/math_ext.py
   ```

3. **Verify plugins are loaded:**
   ```bash
   forb --plugins
   ```

4. **Use plugin functionality:**
   ```bash
   forb "factorial(10)"    # → 3628800
   forb "PI * 2"           # → 6.283185307179586
   forb "fib(20)"          # → 6765
   ```

## Requirements

Plugin support requires **Python 3.8+** to be installed on your system. The `forb` binary links dynamically to Python at runtime.

Pre-built binaries from Homebrew, Scoop, and GitHub releases include plugin support. If you installed via these methods, plugins should work out of the box.

### Building from Source

If building from source, use the `plugins` feature flag:

```bash
# Debug build
cargo build -p formatorbit-cli --features plugins

# Release build
cargo build -p formatorbit-cli --features plugins --release

# Run directly
cargo run -p formatorbit-cli --features plugins -- "factorial(5)"
```

**Note:** On systems with Python 3.14+, you may need:
```bash
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 cargo build -p formatorbit-cli --features plugins
```

## Plugin Directory

Plugins are Python files (`.py`) stored in the plugin directory:

| Platform | Location |
|----------|----------|
| macOS | `~/Library/Application Support/forb/plugins/` |
| Linux | `~/.config/forb/plugins/` |
| Windows | `%APPDATA%\forb\plugins\` |

**Important:** Files ending in `.sample` are not loaded. This follows the git hooks convention—rename to `.py` to enable.

```bash
# Show plugin directory path
forb --plugins path

# Create directory if it doesn't exist
mkdir -p "$(forb --plugins path)"
```

## Plugin Types

### Decoders

Decoders parse input strings into interpretations. Use them for custom data formats.

```python
import forb
from forb import CoreValue, Interpretation

@forb.decoder(
    id="myformat",           # Unique identifier
    name="My Custom Format", # Human-readable name
    aliases=["mf", "myf"]    # Optional short aliases
)
def decode_myformat(input_str):
    """
    Parse strings with 'MYF:' prefix.

    Example:
        forb "MYF:hello" → My Custom Format: hello
    """
    prefix = "MYF:"
    if not input_str.startswith(prefix):
        return []  # Return empty list if format doesn't match

    content = input_str[len(prefix):]

    return [Interpretation(
        value=CoreValue.String(content),
        confidence=0.95,  # 0.0 to 1.0
        description=f"My Custom Format: {content}"
    )]
```

**Key points:**
- Return `[]` if the input doesn't match your format
- Use appropriate confidence scores (higher = more certain)
- Multiple interpretations can be returned

### Expression Variables

Add constants to the expression evaluator.

```python
import forb
import math

@forb.expr_var("PI", description="Mathematical constant pi")
def pi():
    return math.pi

@forb.expr_var("E", description="Euler's number")
def euler():
    return math.e

@forb.expr_var("PHI", description="Golden ratio")
def phi():
    return (1 + math.sqrt(5)) / 2
```

**Usage:**
```bash
forb "PI * 2"        # → 6.283185307179586
forb "E ^ 2"         # → 7.3890560989306495
forb "PHI * 100"     # → 161.80339887498948
```

### Expression Functions

Add functions to the expression evaluator.

```python
import forb
import math

@forb.expr_func("factorial", description="Calculate n!")
def factorial(n):
    return math.factorial(int(n))

@forb.expr_func("fib", description="Fibonacci number at position n")
def fibonacci(n):
    n = int(n)
    if n <= 1:
        return n
    a, b = 0, 1
    for _ in range(n - 1):
        a, b = b, a + b
    return b

@forb.expr_func("gcd", description="Greatest common divisor")
def gcd(a, b):
    return math.gcd(int(a), int(b))

@forb.expr_func("isPrime", description="Check if n is prime (returns 1 or 0)")
def is_prime(n):
    n = int(n)
    if n < 2:
        return 0
    for i in range(2, int(n**0.5) + 1):
        if n % i == 0:
            return 0
    return 1
```

**Usage:**
```bash
forb "factorial(10)"     # → 3628800
forb "fib(20)"           # → 6765
forb "gcd(48, 18)"       # → 6
forb "isPrime(17)"       # → 1
```

### Traits

Traits observe properties of values without transforming them. They appear as checkmarks (✓) in the output.

```python
import forb
import re

@forb.trait(
    id="semver",
    name="Semantic Version",
    value_types=["string"]  # Only check string values
)
def check_semver(value):
    """Detect semantic versioning strings."""
    if not isinstance(value, str):
        return None

    pattern = r'^v?(\d+)\.(\d+)\.(\d+)(?:-([a-zA-Z0-9.-]+))?$'
    match = re.match(pattern, value.strip())

    if not match:
        return None  # Trait doesn't apply

    major, minor, patch = match.group(1), match.group(2), match.group(3)
    return f"Semantic Version: {major}.{minor}.{patch}"

@forb.trait(
    id="well-known-port",
    name="Well-Known Port",
    value_types=["int"]  # Only check integer values
)
def check_port(value):
    """Detect well-known network ports."""
    ports = {22: "SSH", 80: "HTTP", 443: "HTTPS", 3306: "MySQL", 5432: "PostgreSQL"}

    if value in ports:
        return f"Well-Known Port: {ports[value]} ({value}/tcp)"
    return None
```

**Usage:**
```bash
forb "1.2.3"
# → ✓ Semantic Version: 1.2.3

forb "443"
# → ✓ HTTPS (port 443/tcp), Well-Known Port: HTTPS (443/tcp)
```

**value_types options:** `"int"`, `"float"`, `"string"`, `"bytes"`, `"bool"`, `"datetime"`, `"json"`

Leave empty `[]` to check all value types.

### Visualizers

Visualizers provide custom rich display for values (used by GUI applications).

```python
import forb
from forb import RichDisplay, TreeNode

@forb.visualizer(
    id="json-tree",
    name="JSON Tree View",
    value_types=["json"]
)
def visualize_json(value):
    """Render JSON as a tree structure."""
    if not isinstance(value, dict):
        return None

    def build_tree(obj, label="root"):
        if isinstance(obj, dict):
            children = [build_tree(v, k) for k, v in obj.items()]
            return TreeNode(label, None, children)
        elif isinstance(obj, list):
            children = [build_tree(v, f"[{i}]") for i, v in enumerate(obj)]
            return TreeNode(label, None, children)
        else:
            return TreeNode(label, str(obj), [])

    return RichDisplay.Tree(build_tree(value))
```

**Available RichDisplay types:**
- `RichDisplay.KeyValue(pairs)` - Key-value table
- `RichDisplay.Table(headers, rows)` - Data table
- `RichDisplay.Tree(root)` - Tree structure
- `RichDisplay.Color(r, g, b, a)` - Color swatch
- `RichDisplay.Code(language, content)` - Syntax-highlighted code
- `RichDisplay.Map(lat, lon, label)` - Geographic location
- `RichDisplay.Duration(millis, human)` - Time duration
- `RichDisplay.DateTime(epoch_millis, iso, relative)` - Timestamp
- `RichDisplay.DataSize(bytes, human)` - File/data size
- `RichDisplay.Markdown(content)` - Rendered markdown
- `RichDisplay.Progress(value, label)` - Progress indicator

### Currencies

Add custom currency exchange rates.

```python
import forb
import json
from urllib.request import urlopen

@forb.currency(
    code="BTC",
    symbol="\u20bf",  # ₿
    name="Bitcoin",
    decimals=8
)
def btc_rate():
    """
    Get current Bitcoin to USD exchange rate.
    Return None if rate is unavailable.
    """
    try:
        url = "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd"
        with urlopen(url, timeout=5) as response:
            data = json.loads(response.read().decode())
            return data["bitcoin"]["usd"]
    except Exception:
        return None  # Rate unavailable
```

## Plugin Structure

Every plugin file must have a `__forb_plugin__` metadata dictionary:

```python
__forb_plugin__ = {
    "name": "My Plugin",           # Required: Human-readable name
    "version": "1.0.0",            # Required: Semantic version
    "author": "Your Name",         # Optional: Author name
    "description": "What it does"  # Optional: Brief description
}
```

**Complete plugin template:**

```python
# my_plugin.py - Description of what this plugin does
#
# To enable: place in ~/.config/forb/plugins/

__forb_plugin__ = {
    "name": "My Plugin",
    "version": "1.0.0",
    "author": "Your Name",
    "description": "Brief description of functionality"
}

import forb
from forb import CoreValue, Interpretation, RichDisplay, TreeNode

# Add your decorators here...
```

## CLI Commands

```bash
# List all loaded plugins
forb --plugins

# Show detailed status (including errors)
forb --plugins status

# Show plugin directory path
forb --plugins path
```

**Example output:**
```
Loaded Plugins

  ▶ Decoders:
    → example-format
    → rot13-encoded
  ▶ Expression Variables:
    → PI
    → E
    → PHI
    → TAU
  ▶ Expression Functions:
    → factorial()
    → fib()
    → gcd()
    → lcm()

✓ 12 plugin(s) loaded
```

## Configuration

Add plugin settings to `~/.config/forb/config.toml`:

```toml
[plugins]
# Enable/disable plugin loading (default: true)
enabled = true

# Additional plugin directories (beyond the default)
paths = ["/usr/local/share/forb/plugins", "~/myproject/.forb-plugins"]

# Disable specific plugins by ID
disabled = ["plugin-id-to-skip"]
```

**Environment variable:** Set `FORB_PLUGINS=0` to disable all plugins.

## Sample Plugins

The `sample-plugins/` directory contains example plugins:

| File | Description |
|------|-------------|
| `math_ext.py.sample` | Mathematical constants (PI, E, PHI, TAU) and functions (factorial, fib, gcd, lcm, sqrt, sin, cos, etc.) |
| `custom_decoder.py.sample` | Example custom decoder + ROT13 decoder |
| `crypto_rates.py.sample` | Cryptocurrency rates (BTC, ETH, SOL) from CoinGecko |
| `dev_traits.py.sample` | Developer traits: AWS regions, semver, ports, HTTP status codes |

**Enable all sample plugins:**
```bash
cd ~/.config/forb/plugins
for f in /path/to/formatorbit/sample-plugins/*.sample; do
    cp "$f" "$(basename "${f%.sample}")"
done
```

## API Reference

### CoreValue Types

```python
from forb import CoreValue

CoreValue.Empty()                          # No value
CoreValue.Bytes(data)                      # bytes
CoreValue.String(value)                    # str
CoreValue.Int(value, original_bytes=None)  # int
CoreValue.Float(value)                     # float
CoreValue.Bool(value)                      # bool
CoreValue.DateTime(value)                  # ISO 8601 string
CoreValue.Json(data)                       # dict or list
CoreValue.Currency(amount, code)           # {"amount": float, "code": str}
CoreValue.Coordinates(lat, lon)            # {"lat": float, "lon": float}
CoreValue.Length(meters)                   # float (meters)
CoreValue.Weight(grams)                    # float (grams)
CoreValue.Temperature(kelvin)              # float (kelvin)
```

### Interpretation

```python
from forb import Interpretation, CoreValue

Interpretation(
    value=CoreValue.String("hello"),  # The parsed value
    confidence=0.95,                   # 0.0 to 1.0
    description="Human-readable text", # Shown in CLI output
    rich_display=[]                    # Optional RichDisplay list
)
```

### Decorators

```python
import forb

@forb.decoder(id, name, aliases=[])
@forb.expr_var(name, description="")
@forb.expr_func(name, description="")
@forb.trait(id, name, value_types=[])
@forb.visualizer(id, name, value_types=[])
@forb.currency(code, symbol, name, decimals=2)
```

## Troubleshooting

### Plugin not loading

1. Check the file is in the correct directory:
   ```bash
   forb --plugins path
   ```

2. Ensure the file doesn't end in `.sample`

3. Verify the `__forb_plugin__` metadata exists

4. Check for Python errors:
   ```bash
   forb --plugins status
   ```

### Expression function not found

1. Verify the plugin is loaded:
   ```bash
   forb --plugins
   ```

2. Check function name matches exactly (case-sensitive)

3. Ensure function is decorated with `@forb.expr_func`

### Trait not appearing

1. Check `value_types` matches the value being checked

2. Verify the trait function returns a string (not `None`) when it matches

3. Make sure the base format is detecting the input (traits run on interpreted values)

### Python errors

Plugin exceptions are caught and logged. Use verbose mode to see details:
```bash
RUST_LOG=debug forb --plugins status
```

### Build errors

If you get pyo3/Python compatibility errors:
```bash
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 cargo build -p formatorbit-cli --features plugins
```

## Security Considerations

- Plugins run with the same permissions as forb
- Only install plugins from trusted sources
- Review plugin code before enabling
- Currency plugins may make network requests
- Plugin errors are isolated and won't crash forb
