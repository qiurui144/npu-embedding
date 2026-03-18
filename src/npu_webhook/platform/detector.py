"""NPU/iGPU 硬件检测"""

import logging
import subprocess
from pathlib import Path

from npu_webhook.platform.base import NPUDevice

logger = logging.getLogger(__name__)


def _check_intel_npu() -> NPUDevice | None:
    """检测 Intel NPU (Meteor Lake / Lunar Lake / Arrow Lake)"""
    # Linux: /dev/accel* 设备节点
    accel_devs = list(Path("/dev").glob("accel*"))
    if accel_devs:
        try:
            import openvino  # noqa: F401
            return NPUDevice(name="Intel NPU", device_type="npu", vendor="intel", driver="openvino")
        except ImportError:
            logger.info("Intel NPU hardware detected but openvino not installed")
            return NPUDevice(name="Intel NPU (no driver)", device_type="npu", vendor="intel", driver="")

    # Windows: 通过 openvino 查询
    try:
        from openvino import Core
        core = Core()
        if "NPU" in core.available_devices:
            return NPUDevice(name="Intel NPU", device_type="npu", vendor="intel", driver="openvino")
    except Exception:
        pass

    return None


def _check_amd_npu() -> NPUDevice | None:
    """检测 AMD XDNA NPU (Ryzen AI)"""
    # Linux: /dev/amdxdna* 或 /dev/accel* (amdxdna driver)
    xdna_devs = list(Path("/dev").glob("amdxdna*"))
    if xdna_devs:
        return NPUDevice(name="AMD XDNA NPU", device_type="npu", vendor="amd", driver="xdna")

    # Windows: 通过 DirectML 检测
    try:
        import onnxruntime as ort
        providers = ort.get_available_providers()
        if "DmlExecutionProvider" in providers:
            # 有 DirectML，但不一定有 NPU — 先标记为可能
            return NPUDevice(name="AMD NPU (DirectML)", device_type="npu", vendor="amd", driver="directml")
    except Exception:
        pass

    return None


def _check_intel_igpu() -> NPUDevice | None:
    """检测 Intel 集成显卡"""
    try:
        from openvino import Core
        core = Core()
        if "GPU" in core.available_devices:
            return NPUDevice(name="Intel iGPU", device_type="igpu", vendor="intel", driver="openvino")
    except Exception:
        pass

    # Linux: 检查 /dev/dri/renderD* + lspci
    render_devs = list(Path("/dev/dri").glob("renderD*")) if Path("/dev/dri").exists() else []
    if render_devs:
        try:
            result = subprocess.run(["lspci"], capture_output=True, text=True, timeout=5)
            if "Intel" in result.stdout and ("VGA" in result.stdout or "Display" in result.stdout):
                return NPUDevice(name="Intel iGPU", device_type="igpu", vendor="intel", driver="i915")
        except Exception:
            pass

    return None


def _check_amd_igpu() -> NPUDevice | None:
    """检测 AMD 集成显卡"""
    # Linux: ROCm
    try:
        import onnxruntime as ort
        if "ROCMExecutionProvider" in ort.get_available_providers():
            return NPUDevice(name="AMD Radeon iGPU", device_type="igpu", vendor="amd", driver="rocm")
    except Exception:
        pass

    # Linux: 检查 lspci
    try:
        result = subprocess.run(["lspci"], capture_output=True, text=True, timeout=5)
        for line in result.stdout.splitlines():
            if "AMD" in line and ("VGA" in line or "Display" in line):
                return NPUDevice(name="AMD Radeon iGPU", device_type="igpu", vendor="amd", driver="amdgpu")
    except Exception:
        pass

    return None


def _check_ollama() -> NPUDevice | None:
    """检测 Ollama 服务（作为计算后端）"""
    import urllib.request
    try:
        with urllib.request.urlopen("http://localhost:11434/api/tags", timeout=3) as resp:
            if resp.status == 200:
                return NPUDevice(name="Ollama", device_type="ollama", vendor="ollama", driver="http")
    except Exception:
        pass
    return None


def detect_all_devices() -> list[NPUDevice]:
    """检测所有可用计算设备"""
    devices = []
    for checker in [_check_intel_npu, _check_amd_npu, _check_intel_igpu, _check_amd_igpu, _check_ollama]:
        try:
            dev = checker()
            if dev:
                devices.append(dev)
        except Exception as e:
            logger.debug("Device check failed: %s", e)
    # CPU fallback 始终可用
    devices.append(NPUDevice(name="CPU", device_type="cpu", vendor="generic"))
    return devices


def detect_best_device() -> NPUDevice:
    """检测并返回最优计算设备

    优先级:
    1. Ollama（如果可用，直接用 HTTP API 最简单）
    2. Intel NPU (OpenVINO NPU plugin)
    3. AMD XDNA NPU (onnxruntime-directml)
    4. Intel iGPU (OpenVINO GPU plugin)
    5. AMD Radeon iGPU (onnxruntime-rocm)
    6. CPU fallback
    """
    devices = detect_all_devices()
    priority = {"ollama": 0, "npu": 1, "igpu": 2, "cpu": 9}
    devices.sort(key=lambda d: priority.get(d.device_type, 5))
    best = devices[0]
    logger.info("Best device: %s (%s/%s)", best.name, best.device_type, best.vendor)
    return best


def device_to_engine_params(device: NPUDevice) -> dict:
    """将设备信息映射为 create_embedding_engine 参数"""
    if device.device_type == "ollama":
        return {"device": "ollama"}
    if device.vendor == "intel" and device.driver == "openvino":
        return {"device": "openvino"}
    if device.vendor == "amd" and device.driver in ("directml", "xdna"):
        return {"device": "directml"}
    if device.vendor == "amd" and device.driver == "rocm":
        return {"device": "rocm"}
    return {"device": "cpu"}
