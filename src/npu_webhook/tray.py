"""系统托盘入口：pystray + uvicorn 子线程"""
import logging
import threading
import time
from typing import Any

logger = logging.getLogger(__name__)


def _create_icon() -> Any:
    """创建简单的绿色圆形托盘图标"""
    from PIL import Image, ImageDraw

    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw.ellipse([8, 8, 56, 56], fill=(76, 175, 80, 255))  # 绿色
    return img


def _start_server(stop_event: threading.Event) -> None:
    """在子线程中运行 uvicorn"""
    import uvicorn

    from npu_webhook.config import settings
    from npu_webhook.main import app

    config = uvicorn.Config(
        app=app,
        host=settings.server.host,
        port=settings.server.port,
        log_level="info",
    )
    server = uvicorn.Server(config)

    # 监听 stop_event，优雅关闭
    def _check_stop() -> None:
        while not stop_event.is_set():
            time.sleep(1)
        server.should_exit = True

    threading.Thread(target=_check_stop, daemon=True).start()
    server.run()


def main() -> None:
    """系统托盘主进程：pystray 托盘 + uvicorn 子线程"""
    import pystray
    from pystray import MenuItem as item

    stop_event = threading.Event()

    # 启动 uvicorn 子线程
    server_thread = threading.Thread(
        target=_start_server, args=(stop_event,), daemon=True, name="uvicorn"
    )
    server_thread.start()

    def on_quit(icon: Any, item_: Any) -> None:
        stop_event.set()
        icon.stop()

    def on_open_browser(icon: Any, item_: Any) -> None:
        import webbrowser

        webbrowser.open("http://localhost:18900")

    icon = pystray.Icon(
        "npu-webhook",
        _create_icon(),
        "npu-webhook",
        menu=pystray.Menu(
            item("打开状态页", on_open_browser),
            item("退出", on_quit),
        ),
    )
    logger.info("System tray started")
    icon.run()


if __name__ == "__main__":
    main()
