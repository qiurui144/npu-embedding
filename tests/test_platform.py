"""跨平台抽象测试"""

import platform


def test_get_platform_provider():
    from npu_webhook.platform.paths import get_platform_provider

    provider = get_platform_provider()
    system = platform.system()
    if system == "Linux":
        from npu_webhook.platform.linux import LinuxPlatformProvider
        assert isinstance(provider, LinuxPlatformProvider)
    elif system == "Windows":
        from npu_webhook.platform.windows import WindowsPlatformProvider
        assert isinstance(provider, WindowsPlatformProvider)


def test_detect_best_device():
    from npu_webhook.platform.detector import detect_best_device

    device = detect_best_device()
    assert device.device_type == "cpu"
    assert device.vendor == "generic"


def test_data_dir_exists():
    from npu_webhook.platform.paths import get_platform_provider

    provider = get_platform_provider()
    data_dir = provider.get_data_dir()
    assert data_dir is not None
    assert str(data_dir).endswith("npu-webhook")
