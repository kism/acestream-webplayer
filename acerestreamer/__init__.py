"""Flask webapp acerestreamer."""

from pathlib import Path
from pprint import pformat

from . import authentication_bp, config, info_bp, logger, stream_bp
from .flask_helpers import FlaskAceReStreamer

__version__ = "0.2.0"  # This is the version of the app, used in pyproject.toml, enforced in a test.
PROGRAM_NAME = "Ace ReStreamer"  # This is the name of the app, used in the config file.
URL = "https://github.com/kism/ace-restreamer"


def create_app(
    test_config: config.AceReStreamerConf | None = None,
    instance_path: str | None = None,
) -> FlaskAceReStreamer:
    """Create and configure an instance of the Flask application."""
    app = FlaskAceReStreamer(__name__, instance_relative_config=True, instance_path=instance_path)
    app.logger.handlers.clear()

    logger.setup_logger(in_loggers=[])  # Setup flask logger with defaults

    if test_config:  # For Python testing we will often pass in a config
        if not instance_path:
            app.logger.critical("When testing supply both test_config and instance_path!")
            raise AttributeError(instance_path)
        app.aw_conf = test_config

    if app.static_folder:
        try:
            with (Path(app.static_folder) / "favicon.ico").open() as f:
                if "version " in f.read():
                    app.logger.error(
                        "The favicon.ico file is a Git LFS pointer file, the web fonts are probably also wrong too.\n"
                        "Please run 'git lfs install' 'git lfs pull' to download the actual file."
                    )
        except UnicodeDecodeError:
            pass  # All good, not a pointer file

    app.logger.debug("Instance path is: %s", app.instance_path)

    logger.setup_logger(  # Setup logger with config
        log_level=app.aw_conf.logging.level,
        log_path=app.aw_conf.logging.path,
        in_loggers=[],
    )

    # Flask config, at the root of the config object.
    app.config.from_mapping(app.aw_conf.flask.model_dump())

    # Do some debug logging of config
    app_config_str = ">>>\nFlask config:"
    for key, value in app.config.items():
        app_config_str += f"\n  {key}: {pformat(value)}"

    app.logger.debug(app_config_str)

    app.register_blueprint(stream_bp.bp)
    app.register_blueprint(authentication_bp.bp)
    app.register_blueprint(info_bp.bp)

    with app.app_context():
        stream_bp.start_scraper()
        authentication_bp.start_allowlist()

    app.logger.info("Starting Web Server")
    app.logger.info("%s version: %s", PROGRAM_NAME, __version__)

    return app
