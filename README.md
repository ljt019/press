# Press 🍇

**Press** is a command-line tool designed to process and combine text files from specified directories, query the DeepSeek API with a custom prompt, and save the AI-generated response. It's particularly useful for developers who want to automate the processing of multiple code files and get AI-generated insights or modifications.

## Features

- **File Aggregation**: Combines text files from multiple directories into a single input for the AI.
- **DeepSeek Integration**: Queries the DeepSeek API with a custom prompt and retrieves the AI's response.
- **Output Management**: Saves the AI-generated response to a specified output directory.
- **API Key Management**: Stores the DeepSeek API key for future use, so you don't need to provide it every time.

## Installation

1. **Clone the Repository**:
   ```bash
   git clone https://github.com/yourusername/press.git
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

To process files from one or more directories and query the DeepSeek API:

```bash
press --directories path/to/dir1 path/to/dir2 --prompt "Your custom prompt here" --api-key YOUR_API_KEY
```

### Options

- `--directories`: Specify one or more directories to process. Separate multiple directories with a space.
- `--output-directory`: Specify the output directory where the results will be saved. Defaults to `./`.
- `--prompt`: Provide a custom prompt for the AI.
- `--api-key`: Provide your DeepSeek API key. This is only required the first time you run the tool.

### Example

```bash
press --directories src tests --prompt "Refactor the code for better readability" --output-directory ./output
```

## Acknowledgments

- **DeepSeek**: For providing the AI API that powers this tool.
- **Rust Community**: For the amazing ecosystem and libraries that made this project possible.

---

🍇 **Press** - Squeeze the most out of your code! 🍇
