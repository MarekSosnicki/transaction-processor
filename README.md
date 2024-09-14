# Transaction processor

This is a simple CLI application that processes a list of transactions in CSV format and returns the account balance
for each client.

## Running

To run the code use the following command:

```bash
cargo run -- INPUT
```

Where `INPUT` should be a path to the input `.csv` file.
The output of the application is a CSV with a summary of all client accounts after performing transactions written to
stdout.

## Testing

To run tests use the following command:

```bash
cargo test
```

## Examples

You can find example inputs and outputs in the `test_data` directory.

## Design choices

A few highlights on the design choices:

- There are two main components of the application:
    - CSV processing part - that is handling the input and output, it is strictly tied to the CLI nature of the project
    - Transaction processor - that is responsible for all the business transaction logic, it is written in a way that
      abstracts it from the input and output types so this module could be reused e.g. if one would like to put it e.g.,
      behind an API
- To ensure 4 digits precision of calculation, internally I use integers (round the floats to the integers)
- Transaction processor logic returns errors, but the application treats all failed transactions ignored