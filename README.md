# Press 🍇

Press is a CLI tool that batches text files, sends them to DeepSeek's AI with your custom prompt, and saves the response. Perfect for developers seeking AI-assisted code analysis and modifications at scale.

## Features

- **File Aggregation**: Process multiple files and directories in a single command
- **DeepSeek Integration**: Query the AI with custom prompts and system messages
- **Smart Output**: Save responses with intelligent file parsing and organization
- **Auto Mode**: Optionally overwrite original files with AI-generated content
- **API Key Management**: Securely store your DeepSeek API key for future use

## Installation

```bash
# Clone the repository
git clone https://github.com/ljt019/press.git
cd press

# Build the project
cargo build --release
```

### Adding to PATH (Windows)

Option 1: Using PowerShell (temporary)
```powershell
$Env:PATH += ";$PWD\target\release"
```

Option 2: Through Windows Settings (permanent)
1. Press `Win + X` and select "System"
2. Click "About" then "Advanced system settings"
3. Click "Environment Variables"
4. Under "User variables", select "Path" and click "Edit"
5. Click "New"
6. Add the full path to the `press.exe` (e.g., `C:\Users\YourName\projects\press\target\release`)
7. Click "OK" on all windows

Option 3: Using Command Prompt (permanent)
```cmd
setx PATH "%PATH%;C:\path\to\press\target\release"
```

Verify the installation by opening a new terminal and running:
```bash
press --help
```

## Usage

### Basic Command

```bash
press --paths src/lib.rs tests/ --prompt "Add comprehensive tests" --api-key YOUR_API_KEY
```

### Options

- `--paths`: Files or directories to process (space-separated)
- `--output-directory`: Where to save results (default: `./`)
- `--prompt`: Your instruction for the AI
- `--system-prompt`: Custom AI system message (default: "You are a helpful assistant")
- `--api-key`: DeepSeek API key (only needed first time)
- `--auto`: Overwrite original files with AI output

### Examples

Refactor Code:
```bash
press --paths src tests --prompt "Refactor for better readability" --api-key YOUR_API_KEY
```

Save to Custom Directory:
```bash
press --paths src utils --prompt "Optimize performance" --output-directory ./optimized
```

Auto-Update Files:
```bash
press --paths src --prompt "Add documentation" --auto
```

Custom System Prompt:
```bash
press --paths project/src --prompt "Improve error handling" --system-prompt "You are a senior engineer"
```

🍇 **Press** - Squeeze the most out of your code!