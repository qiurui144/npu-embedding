"""NPU/iGPU 硬件检测 + 内核/驱动/软件栈检查"""

import logging
import platform
import subprocess
from dataclasses import dataclass, field
from pathlib import Path

from npu_webhook.platform.base import NPUDevice

logger = logging.getLogger(__name__)


@dataclass
class DriverCheck:
    """驱动/软件栈检查结果"""
    name: str
    installed: bool
    version: str = ""
    message: str = ""


@dataclass
class PlatformReport:
    """完整平台检测报告"""
    os: str = ""
    kernel: str = ""
    arch: str = ""
    devices: list[NPUDevice] = field(default_factory=list)
    drivers: list[DriverCheck] = field(default_factory=list)
    install_commands: list[str] = field(default_factory=list)


# ---------------------------------------------------------------------------
# 硬件检测
# ---------------------------------------------------------------------------


def _check_intel_npu() -> NPUDevice | None:
    """检测 Intel NPU (Meteor Lake / Lunar Lake / Arrow Lake)"""
    accel_devs = list(Path("/dev").glob("accel*"))
    if accel_devs:
        try:
            import openvino  # noqa: F401
            return NPUDevice(name="Intel NPU", device_type="npu", vendor="intel", driver="openvino")
        except ImportError:
            return NPUDevice(name="Intel NPU (no driver)", device_type="npu", vendor="intel", driver="")

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
    xdna_devs = list(Path("/dev").glob("amdxdna*"))
    if xdna_devs:
        return NPUDevice(name="AMD XDNA NPU", device_type="npu", vendor="amd", driver="xdna")

    try:
        import onnxruntime as ort
        if "DmlExecutionProvider" in ort.get_available_providers():
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
    try:
        import onnxruntime as ort
        if "ROCMExecutionProvider" in ort.get_available_providers():
            return NPUDevice(name="AMD Radeon iGPU", device_type="igpu", vendor="amd", driver="rocm")
    except Exception:
        pass

    try:
        result = subprocess.run(["lspci"], capture_output=True, text=True, timeout=5)
        for line in result.stdout.splitlines():
            if "AMD" in line and ("VGA" in line or "Display" in line):
                return NPUDevice(name="AMD Radeon iGPU", device_type="igpu", vendor="amd", driver="amdgpu")
    except Exception:
        pass
    return None


def _check_ollama() -> NPUDevice | None:
    """检测 Ollama 服务"""
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
    devices.append(NPUDevice(name="CPU", device_type="cpu", vendor="generic"))
    return devices


def detect_best_device() -> NPUDevice:
    """检测并返回最优计算设备"""
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


# ---------------------------------------------------------------------------
# 内核/驱动/软件栈深度检查
# ---------------------------------------------------------------------------


def _run_cmd(cmd: list[str], timeout: int = 5) -> str:
    """运行命令并返回 stdout，失败返回空字符串"""
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        return r.stdout.strip() if r.returncode == 0 else ""
    except Exception:
        return ""


def _check_kernel_module(module: str) -> bool:
    """检查内核模块是否已加载"""
    return module in _run_cmd(["lsmod"])


def check_kernel_info() -> dict:
    """获取内核信息"""
    return {
        "system": platform.system(),
        "kernel": platform.release(),
        "arch": platform.machine(),
        "version": platform.version(),
    }


def check_intel_drivers() -> list[DriverCheck]:
    """检查 Intel NPU/iGPU 驱动栈"""
    checks = []
    system = platform.system()

    if system == "Linux":
        # 内核模块: intel_vpu (NPU)
        intel_vpu = _check_kernel_module("intel_vpu")
        checks.append(DriverCheck(
            name="intel_vpu 内核模块",
            installed=intel_vpu,
            message="Intel NPU 驱动" if intel_vpu else "未加载 — 需要 Linux 6.3+ 内核",
        ))

        # 内核模块: i915 (iGPU)
        i915 = _check_kernel_module("i915")
        checks.append(DriverCheck(
            name="i915 内核模块",
            installed=i915,
            message="Intel iGPU 驱动" if i915 else "未加载",
        ))

        # NPU 固件
        npu_fw = Path("/lib/firmware/intel/vpu").exists() or Path("/lib/firmware/intel/npu").exists()
        checks.append(DriverCheck(
            name="Intel NPU 固件",
            installed=npu_fw,
            message="已安装" if npu_fw else "未找到 — apt install intel-npu-firmware",
        ))

        # /dev/accel* 设备节点
        accel = bool(list(Path("/dev").glob("accel*")))
        checks.append(DriverCheck(
            name="/dev/accel 设备节点",
            installed=accel,
            message="可用" if accel else "不可用 — 内核需支持 DRM accel 子系统",
        ))

        # Level Zero 运行时
        l0 = bool(_run_cmd(["dpkg", "-l", "level-zero"]))
        checks.append(DriverCheck(
            name="Level Zero 运行时",
            installed=l0,
            message="已安装" if l0 else "未安装 — apt install level-zero intel-level-zero-gpu",
        ))

    # Python: openvino
    try:
        import openvino
        checks.append(DriverCheck(name="openvino", installed=True, version=openvino.__version__))
    except ImportError:
        checks.append(DriverCheck(name="openvino", installed=False, message="pip install openvino"))

    return checks


def check_amd_drivers() -> list[DriverCheck]:
    """检查 AMD NPU/iGPU 驱动栈"""
    checks = []
    system = platform.system()

    if system == "Linux":
        # 内核模块: amdxdna (NPU)
        amdxdna = _check_kernel_module("amdxdna")
        checks.append(DriverCheck(
            name="amdxdna 内核模块",
            installed=amdxdna,
            message="AMD XDNA NPU 驱动" if amdxdna else "未加载 — 需要 amdxdna-dkms 包",
        ))

        # 内核模块: amdgpu (iGPU)
        amdgpu = _check_kernel_module("amdgpu")
        checks.append(DriverCheck(
            name="amdgpu 内核模块",
            installed=amdgpu,
            message="AMD GPU 驱动" if amdgpu else "未加载",
        ))

        # /dev/amdxdna* 设备节点
        xdna_dev = bool(list(Path("/dev").glob("amdxdna*")))
        checks.append(DriverCheck(
            name="/dev/amdxdna 设备节点",
            installed=xdna_dev,
            message="可用" if xdna_dev else "不可用",
        ))

        # ROCm
        rocm = Path("/opt/rocm").exists()
        rocm_ver = _run_cmd(["cat", "/opt/rocm/.info/version"]) if rocm else ""
        checks.append(DriverCheck(
            name="ROCm",
            installed=rocm,
            version=rocm_ver,
            message="已安装" if rocm else "未安装 — 参考 AMD ROCm 安装指南",
        ))

    # Python: onnxruntime-directml / rocm
    try:
        import onnxruntime as ort
        providers = ort.get_available_providers()
        has_dml = "DmlExecutionProvider" in providers
        has_rocm = "ROCMExecutionProvider" in providers
        ver = ort.__version__
        if has_dml:
            checks.append(DriverCheck(name="onnxruntime-directml", installed=True, version=ver))
        elif has_rocm:
            checks.append(DriverCheck(name="onnxruntime-rocm", installed=True, version=ver))
        else:
            checks.append(DriverCheck(
                name="onnxruntime (AMD 加速)",
                installed=False,
                message="pip install onnxruntime-directml (Windows) 或 onnxruntime-rocm (Linux)",
            ))
    except ImportError:
        checks.append(DriverCheck(name="onnxruntime", installed=False, message="pip install onnxruntime"))

    return checks


def check_ollama() -> list[DriverCheck]:
    """检查 Ollama 服务"""
    checks = []

    # 服务可达
    import urllib.request
    import json
    try:
        with urllib.request.urlopen("http://localhost:11434/api/tags", timeout=3) as resp:
            data = json.loads(resp.read())
        models = [m["name"] for m in data.get("models", [])]
        checks.append(DriverCheck(
            name="Ollama 服务",
            installed=True,
            message=f"运行中，{len(models)} 个模型",
        ))

        # bge-m3 模型
        has_bge = any("bge-m3" in m for m in models)
        checks.append(DriverCheck(
            name="bge-m3 模型",
            installed=has_bge,
            message="已拉取" if has_bge else "未拉取 — ollama pull bge-m3",
        ))
    except Exception:
        checks.append(DriverCheck(
            name="Ollama 服务",
            installed=False,
            message="不可达 — curl -fsSL https://ollama.com/install.sh | sh",
        ))

    return checks


def generate_install_commands(devices: list[NPUDevice], drivers: list[DriverCheck]) -> list[str]:
    """根据检测结果生成一键安装命令"""
    commands = []
    system = platform.system()
    missing = {d.name for d in drivers if not d.installed}

    if system != "Linux":
        return commands

    # Ollama
    if "Ollama 服务" in missing:
        commands.append("curl -fsSL https://ollama.com/install.sh | sh")
        commands.append("ollama pull bge-m3")
    elif "bge-m3 模型" in missing:
        commands.append("ollama pull bge-m3")

    # Intel NPU 栈
    has_intel_hw = any(d.vendor == "intel" and d.device_type in ("npu", "igpu") for d in devices)
    if has_intel_hw:
        if "Intel NPU 固件" in missing:
            commands.append("sudo apt-get install -y intel-npu-firmware")
        if "Level Zero 运行时" in missing:
            commands.append("sudo apt-get install -y level-zero intel-level-zero-gpu")
        if "openvino" in missing:
            commands.append("pip install openvino")

    # AMD 栈
    has_amd_hw = any(d.vendor == "amd" and d.device_type in ("npu", "igpu") for d in devices)
    if has_amd_hw:
        if "amdxdna 内核模块" in missing:
            commands.append("# AMD XDNA NPU: sudo apt install amdxdna-dkms (需要 AMD 官方源)")
        if "ROCm" in missing:
            commands.append("# AMD ROCm: https://rocm.docs.amd.com/projects/install-on-linux/en/latest/")
        if "onnxruntime (AMD 加速)" in missing or "onnxruntime" in missing:
            commands.append("pip install onnxruntime-directml  # Windows")
            commands.append("# pip install onnxruntime-rocm  # Linux + ROCm")

    return commands


def full_platform_check() -> PlatformReport:
    """完整平台检测报告：硬件 + 内核 + 驱动 + 安装建议"""
    kernel = check_kernel_info()
    devices = detect_all_devices()

    drivers: list[DriverCheck] = []
    vendors = {d.vendor for d in devices}
    if "intel" in vendors:
        drivers.extend(check_intel_drivers())
    if "amd" in vendors:
        drivers.extend(check_amd_drivers())
    drivers.extend(check_ollama())

    install_cmds = generate_install_commands(devices, drivers)

    return PlatformReport(
        os=kernel["system"],
        kernel=kernel["kernel"],
        arch=kernel["arch"],
        devices=devices,
        drivers=drivers,
        install_commands=install_cmds,
    )
