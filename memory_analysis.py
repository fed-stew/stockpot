#!/usr/bin/env python3
"""
Memory usage analyzer for coding tools on macOS.
Captures RSS (actual physical memory) and VSZ (virtual memory) usage.
"""

import subprocess
import re
from dataclasses import dataclass
from typing import List


@dataclass
class ProcessInfo:
    name: str
    pid: int
    rss_mb: float
    vsz_mb: float
    cpu_percent: float
    
    def __str__(self) -> str:
        return (
            f"{self.name:40} | "
            f"PID: {self.pid:6} | "
            f"RSS: {self.rss_mb:8.1f} MB | "
            f"VSZ: {self.vsz_mb:10.1f} MB | "
            f"CPU: {self.cpu_percent:5.1f}%"
        )


def get_process_memory() -> List[ProcessInfo]:
    """Fetch memory usage for all processes."""
    try:
        output = subprocess.check_output(["ps", "aux"], text=True)
    except subprocess.CalledProcessError as e:
        print(f"Error running ps: {e}")
        return []
    
    processes = []
    for line in output.split("\n")[1:]:  # Skip header
        if not line.strip():
            continue
        
        parts = line.split()
        if len(parts) < 11:
            continue
        
        try:
            pid = int(parts[1])
            cpu = float(parts[2])
            vsz = int(parts[4]) / 1024  # Convert KB to MB
            rss = int(parts[5]) / 1024  # Convert KB to MB
            command = " ".join(parts[10:])
            
            # Extract readable name
            if "Antigravity" in command:
                name = "Antigravity"
            elif "code-puppy" in command:
                name = "Code Puppy"
            elif "iTerm" in command:
                name = "iTerm2"
            elif "Chrome" in command:
                name = "Chrome"
            elif "Discord" in command:
                name = "Discord"
            else:
                continue  # Skip non-coding tools
            
            processes.append(ProcessInfo(
                name=name,
                pid=pid,
                rss_mb=rss,
                vsz_mb=vsz,
                cpu_percent=cpu
            ))
        except (ValueError, IndexError):
            continue
    
    return processes


def main():
    """Main entry point."""
    print("\n" + "=" * 120)
    print("CODING TOOLS MEMORY ANALYSIS".center(120))
    print("=" * 120)
    print()
    
    processes = get_process_memory()
    
    if not processes:
        print("No coding tools detected running.")
        return
    
    # Sort by RSS (actual memory) descending
    processes.sort(key=lambda p: p.rss_mb, reverse=True)
    
    print(f"{'Tool':<40} | {'PID':<8} | {'RSS (MB)':<10} | {'VSZ (MB)':<12} | {'CPU %':<7}")
    print("-" * 120)
    
    total_rss = 0
    total_vsz = 0
    
    for proc in processes:
        print(proc)
        total_rss += proc.rss_mb
        total_vsz += proc.vsz_mb
    
    print("-" * 120)
    print(f"{'TOTAL':<40} | {'':<8} | {total_rss:>8.1f} MB | {total_vsz:>10.1f} MB | {'':<7}")
    print("=" * 120)
    print()
    print("Notes:")
    print("  • RSS = Resident Set Size (actual physical RAM being used)")
    print("  • VSZ = Virtual Memory Size (total addressable memory)")
    print("  • CPU % = CPU usage percentage")
    print()


if __name__ == "__main__":
    main()
