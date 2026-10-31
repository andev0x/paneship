# Support

Thanks for using Paneship. Before opening an issue, try the self-service
resources below; they answer most installation, configuration, and daemon
questions.

## Documentation

- [Documentation index](docs/README.md)
- [Installation and shell setup](README.md#installation)
- [Configuration reference](docs/configuration.md)
- [CLI reference](docs/cli.md)
- [Troubleshooting](docs/troubleshooting.md)

## Quick diagnostics

```bash
paneship --version
paneship daemon ping
paneship top --once
tail -n 50 ~/.cache/paneship/daemon.log
```

Include the output in a bug report after redacting credentials, tokens, and
private environment details.

## Asking a question

Use [GitHub Issues](https://github.com/andev0x/paneship/issues) for reproducible
bugs and concrete feature requests. Search existing issues first and use the
provided templates. General usage questions are welcome when they include enough
context to reproduce the situation.

## Reporting a security issue

Do not open a public issue. Follow the [Security Policy](SECURITY.md) and use
GitHub's private vulnerability reporting.

## Contributing a fix

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, the quality gate,
and the pull-request checklist. Documentation fixes and small detector additions
are good first contributions.

## Response expectations

Paneship is maintained by volunteers in their spare time. Reports with a clear
reproduction and diagnostics are prioritized. Security reports follow the
timelines in the [Security Policy](SECURITY.md).
