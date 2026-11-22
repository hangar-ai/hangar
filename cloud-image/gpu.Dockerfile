# syntax=docker/dockerfile:1
#
# Our image for GPU sessions, ready for Vulkan and OpenGL without an image of
# the person's own (docs/native/cloud-builds.md, "GPU sessions"):
# ghcr.io/hangar-ai/hangar-box:<version>-gpu, the image of Dockerfile beside
# this one (BOX), plus what renders without a screen.
#
# The NVIDIA driver is not in it: the provider's NVIDIA container runtime
# mounts the host's, the very version the host runs, its user-space
# libraries included (libGLX_nvidia, libEGL_nvidia, libnvidia-glcore...)
# when the container asks for the graphics capability, which
# NVIDIA_DRIVER_CAPABILITIES below does. A driver in the image would have to
# be the host's to the last digit; the runtime's always is. The machine's
# receipt says which (the GPU's model and driver, from nvidia-smi).
#
# What the image brings:
# - the Vulkan loader, and the files that point it and glvnd at NVIDIA's
#   libraries (the runtime of some hosts mounts them too, at these same
#   paths);
# - Mesa: lavapipe, Vulkan on the CPU, for a job without a GPU or a host
#   whose driver does not answer (VK_ICD_FILENAMES picks one), and RADV;
# - vulkan-tools (vulkaninfo, vkcube), the Khronos validation layers,
#   spirv-tools;
# - Xvfb and Weston (its headless backend), for programs that want a
#   display; gdb.
#
# Built at a release after the image it extends, with the same version:
#
#   docker build --build-arg BOX=ghcr.io/hangar-ai/hangar-box:3.0.0 \
#     -f public/cloud-image/gpu.Dockerfile \
#     -t ghcr.io/hangar-ai/hangar-box:3.0.0-gpu public/cloud-image

ARG BOX=ghcr.io/hangar-ai/hangar-box:dev
FROM ${BOX}

ARG VERSION=dev
LABEL org.opencontainers.image.title="hangar-box-gpu" \
      org.opencontainers.image.description="A HangarAI Cloud Builds machine for GPU sessions: the toolchains, hangar-box, Vulkan and OpenGL ready for the host's NVIDIA driver" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.ref.name="ghcr.io/hangar-ai/hangar-box:${VERSION}-gpu"

ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      libvulkan1 vulkan-tools vulkan-validationlayers spirv-tools \
      mesa-vulkan-drivers libgl1-mesa-dri \
      libglvnd0 libgl1 libglx0 libegl1 libgles2 \
      xvfb xauth weston \
      gdb \
 && apt-get clean \
 && rm -rf /var/lib/apt/lists/* /var/cache/apt/* /var/cache/debconf/*-old \
      /var/log/apt /var/log/dpkg.log /var/log/alternatives.log

# Where the loader and glvnd look for NVIDIA's libraries, which the runtime
# mounts.
RUN mkdir -p /etc/vulkan/icd.d /usr/share/glvnd/egl_vendor.d \
 && printf '%s\n' '{"file_format_version": "1.0.0", "ICD": {"library_path": "libGLX_nvidia.so.0", "api_version": "1.4"}}' \
      > /etc/vulkan/icd.d/nvidia_icd.json \
 && printf '%s\n' '{"file_format_version": "1.0.0", "ICD": {"library_path": "libEGL_nvidia.so.0"}}' \
      > /usr/share/glvnd/egl_vendor.d/10_nvidia.json

# The driver's capabilities the runtime mounts: graphics and display (Vulkan,
# OpenGL, EGL) besides compute and utility (CUDA, nvidia-smi).
ENV NVIDIA_DRIVER_CAPABILITIES=compute,utility,graphics,video,display \
    NVIDIA_VISIBLE_DEVICES=all
