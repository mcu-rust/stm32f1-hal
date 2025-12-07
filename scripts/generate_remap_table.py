import csv
import os
import pprint
import subprocess

from base import Write

DRY_RUN = False
SCRIPT = os.path.relpath(__file__, os.getcwd()).replace("\\", "/")
REMAP_MODES = {
    "DEFAULT": "RemapDefault",
    "PARTIAL_REMAP": "RemapPartial1",
    "PARTIAL_REMAP1": "RemapPartial1",
    "PARTIAL_REMAP2": "RemapPartial2",
    "FULL_REMAP": "RemapFull",
    "REMAP": "RemapFull",
}

CFG_TABLE = {
    "TIM1": '#[cfg(any(feature = "stm32f100", feature = "stm32f103", feature = "connectivity"))]',
    "TIM4": '#[cfg(feature = "medium")]',
    "TIM5": '#[cfg(any(feature = "high", feature = "connectivity"))]',
    "TIM6": '#[cfg(any(feature = "stm32f100", feature = "high", feature = "connectivity"))]',
    "TIM7": '#[cfg(any(all(feature = "high", any(feature = "stm32f101", feature = "stm32f103")),any(feature = "stm32f100", feature = "connectivity")))]',
    "TIM8": '#[cfg(all(feature = "stm32f103", feature = "high"))]',
    "TIM9": '#[cfg(feature = "xl")]',
    "TIM10": '#[cfg(feature = "xl")]',
    "TIM11": '#[cfg(feature = "xl")]',
    "TIM12": '#[cfg(any(feature = "xl", all(feature = "stm32f100", feature = "high",)))]',
    "TIM13": '#[cfg(any(feature = "xl", all(feature = "stm32f100", feature = "high",)))]',
    "TIM14": '#[cfg(any(feature = "xl", all(feature = "stm32f100", feature = "high",)))]',
    "TIM15": '#[cfg(feature = "stm32f100")]',
    "TIM16": '#[cfg(feature = "stm32f100")]',
    "TIM17": '#[cfg(feature = "stm32f100")]',
}


def match_filter(filter: str, name: str) -> bool:
    if filter == "UART":
        return name.startswith("UART") or name.startswith("USART")
    return name.startswith(filter)


def func_pin_name(filter: str, func: str) -> str:
    return filter[0] + filter[1:].lower() + func[0] + func[1:].lower() + "Pin"


REG_TEMPLATE = """impl RemapMode<{peri}> for {mode}<{peri}> {{
    fn remap(afio: &mut Afio) {{
        {op}
    }}
}}
"""


def write_reg_operation(d: dict, filter: str, w: Write) -> None:
    w.write("\n// Register operations ------------\n\n")
    for peri, remap_modes in sorted(d.items()):
        if match_filter(filter, peri):
            for mode_name, mode_info in sorted(remap_modes.items()):
                mode = REMAP_MODES[mode_name]
                reg = mode_info["reg"]
                bits: str = mode_info["bits"]
                if reg == "none":
                    op = ""
                elif peri == "TIM5":
                    b = "set_bit" if bits[2] == "1" else "clear_bit"
                    op = f"afio.{reg}.modify_mapr(|_, w| w.{peri.lower()}ch4_iremap().{b}());"
                elif len(bits) == 3:
                    b = "set_bit" if bits[2] == "1" else "clear_bit"
                    op = f"afio.{reg}.modify_mapr(|_, w| w.{peri.lower()}_remap().{b}());"
                elif len(bits) == 4:
                    b = f"unsafe {{|_, w| w.{peri.lower()}_remap().bits({bits})}}"
                    op = f"afio.{reg}.modify_mapr({b});"
                else:
                    continue
                w.write(CFG_TABLE.get(peri, ""))
                w.write(REG_TEMPLATE.format(mode=mode, peri=peri, op=op))


BINDER_BODY = """ {{
    #[inline(always)]
    fn is_pin(&self) -> bool {{
        {v}
    }}
}}
"""


def write_binder_type(d: dict, filter: str, w: Write) -> None:
    w.write("\n// Binder types ------------------\n\n")
    func_list: list[str] = []
    for peri, remap_modes in d.items():
        if match_filter(filter, peri):
            for mode_info in remap_modes.values():
                for pin_func in mode_info["pins"].keys():
                    func_list.append(pin_func)

    func_list = sorted(list(set(func_list)))
    for func in func_list:
        name = func_pin_name(filter, func)
        w.write(f"pub trait {name}<REMAP>" + BINDER_BODY.format(v="true"))
        w.write(f"impl<T> {name}<T> for NonePin" + BINDER_BODY.format(v="false"))
    w.write("\n")


IMPL_TEMPLATE_LIST = [
    (
        ["UartTxPin", "UartCkPin", "TimCh1Pin", "TimCh2Pin", "TimCh3Pin", "TimCh4Pin"],
        "impl {func}<{mode}<{peri}>> for {pin}<Alternate<PushPull>>",
    ),
    (["UartRxPin"], "impl<PULL: UpMode> {func}<{mode}<{peri}>> for {pin}<Input<PULL>>"),
    (["I2cSclPin", "I2cSdaPin"], "impl {func}<{mode}<{peri}>> for {pin}<Alternate<OpenDrain>>"),
]


def get_impl_template(func: str) -> str:
    for item in IMPL_TEMPLATE_LIST:
        if func in item[0]:
            return item[1]
    return ""


def write_item(filter: str, peri: str, mode: str, pins: dict[str, str], w: Write) -> None:
    for pin_func, pin in sorted(pins.items()):
        func = func_pin_name(filter, pin_func)
        impl = get_impl_template(func)
        if impl:
            cfg = CFG_TABLE.get(peri, "")
            if cfg:
                w.write(cfg)

            w.write(impl.format(func=func, mode=mode, peri=peri, pin=pin))
            w.write("{}")


def write_table(d: dict, filter: str, csv_file: str, target_file: str) -> None:
    with open(target_file, "r", encoding="utf-8") as f:
        code = f.read()
        i = code.find("// table") + len("// table")
        before = code[:i]
        code = code[i:]

    w = Write(target_file, DRY_RUN)
    w.write(before)
    w.write("\n// Do NOT manually modify the code.\n")
    w.write(
        f"// It's generated by {SCRIPT} from {csv_file}\n",
    )
    write_binder_type(d, filter, w)
    w.write("\n// Bind pins ---------------------\n\n")
    for peri, remap_modes in sorted(d.items()):
        if match_filter(filter, peri):
            for mode_name, mode_info in sorted(remap_modes.items()):
                mode = REMAP_MODES[mode_name]
                write_item(filter, peri, mode, mode_info["pins"], w)
    w.write("\n")
    write_reg_operation(d, filter, w)
    w.close()
    subprocess.run(["rustfmt", target_file])


def parse_remap_info(row: list[str], ret_d: dict) -> None:
    peripheral = row[0]
    reg = row[1]
    remap_mode = row[2]
    reg_bits = row[3]
    pins: dict[str, str] = {}
    for pin in row[4:]:
        if pin:
            (func, pin) = pin.split(":")
            if "/" in func:
                (f1, f2) = func.split("/")
                pins[f1] = pin
                pins[f2] = pin
            else:
                pins[func] = pin

    p = ret_d.setdefault(peripheral, {})
    p[remap_mode] = {
        "reg": reg,
        "bits": reg_bits,
        "pins": pins,
    }


def csv_to_code(csv_file: str, show: bool = False) -> None:
    print(csv_file)
    d: dict = {}
    with open(csv_file, newline="", encoding="utf-8") as f:
        reader = csv.reader(f, delimiter=",", quotechar='"')
        for row in reader:
            if row[0]:
                parse_remap_info(row, d)

    if show:
        pprint.pprint(d)

    write_table(d, "UART", csv_file, "src/afio/uart_remap.rs")
    write_table(d, "TIM", csv_file, "src/afio/timer_remap.rs")
    write_table(d, "I2C", csv_file, "src/afio/i2c_remap.rs")


if __name__ == "__main__":
    csv_to_code("scripts/table/stm32f1_remap_peripheral.csv")
