# Press 🍇

**Press** is a command-line tool designed to process and combine text files from specified paths, query the DeepSeek API with a custom prompt, and save the AI-generated response. It's particularly useful for developers who want to automate the processing of multiple code files and get AI-generated insights or modifications.

## Features

- **File Aggregation**: Combines text files from multiple paths (directories or files) into a single input for the AI.
- **DeepSeek Integration**: Queries the DeepSeek API with a custom prompt and retrieves the AI's response.
- **Output Management**: Saves the AI-generated response to a specified output directory.
- **Enhanced Output Parsing**: Parses AI responses tagged with filenames to save individual files accurately.
- **Automatic Overwrite**: Option to automatically overwrite original files with AI-generated content, streamlining workflows.
- **API Key Management**: Stores the DeepSeek API key for future use, so you don't need to provide it every time.

## Installation

1. **Clone the Repository**:
   ```bash
   git clone https://github.com/ljt019/press.git
   cd press
   ```

2. **Build the Project**:
   ```bash
   cargo build --release
   ```

3. **Run the Executable**:
   ```bash
   ./target/release/press --help
   ```

## Usage

### Basic Usage

To process files from one or more paths (directories or files) and query the DeepSeek API:

```bash
press --paths path/to/dir1 path/to/file1 --prompt "Your custom prompt here" --api-key YOUR_API_KEY
```

### Options

- `--paths`: Specify one or more paths (directories or files) to process. Separate multiple paths with a space.
- `--output-directory`: Specify the output directory where the results will be saved. Defaults to `./`.
- `--prompt`: Provide a custom prompt for the AI.
- `--system-prompt`: Provide a custom system prompt for the AI. Defaults to "You are a helpful assistant".
- `--api-key`: Provide your DeepSeek API key. This is only required the first time you run the tool.
- `--auto`: Automatically overwrite original files with AI-generated ones. Use this with caution as it will replace your existing files.

### Example Commands

##### Basic File Processing:
```bash
Copy code
press --paths src tests main.rs --prompt "Refactor the code for better readability" --api-key YOUR_API_KEY
```
##### Saving Outputs to a Specific Directory:
```bash
Copy code
press --paths src utils --prompt "Optimize the performance of these scripts" --output-directory ./optimized_code```

##### Overwriting Original Files:
```bash
Copy code
press --paths src --prompt "Add comprehensive comments to the codebase" --auto```

##### Using a Custom System Prompt:
```bash
Copy code
press --paths project/src --prompt "Improve the error handling mechanisms" --system-prompt "You are a senior software engineer"```

🍇 **Press** - Squeeze the most out of your code! 🍇