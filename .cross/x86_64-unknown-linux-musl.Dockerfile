ARG BOOTSTRAP
FROM ${BOOTSTRAP:-alpine:3.11} as builder

WORKDIR /gentoo

ARG ARCH=amd64
ARG MICROARCH=amd64
ARG SUFFIX=-musl-llvm
ARG DIST="https://ftp-osl.osuosl.org/pub/gentoo/releases/${ARCH}/autobuilds"
ARG SIGNING_KEY="0xBB572E0E2D182910"

RUN apk --no-cache add ca-certificates gnupg tar wget xz \
  && gpg --list-keys \
  && echo "honor-http-proxy" >> ~/.gnupg/dirmngr.conf \
  && echo "disable-ipv6" >> ~/.gnupg/dirmngr.conf \
  && gpg --keyserver hkps://keys.gentoo.org --recv-keys ${SIGNING_KEY} \
  && wget -q "${DIST}/latest-stage3-${MICROARCH}${SUFFIX}.txt" \
  && gpg --verify "latest-stage3-${MICROARCH}${SUFFIX}.txt" \
  && STAGE3PATH="$(sed -n '6p' "latest-stage3-${MICROARCH}${SUFFIX}.txt" | cut -f 1 -d ' ')" \
  && echo "STAGE3PATH:" ${STAGE3PATH} \
  && STAGE3="$(basename ${STAGE3PATH})" \
  && wget -q "${DIST}/${STAGE3PATH}" "${DIST}/${STAGE3PATH}.CONTENTS.gz" "${DIST}/${STAGE3PATH}.asc" \
  && gpg --verify "${STAGE3}.asc" \
  && tar xpf "${STAGE3}" --xattrs-include='*.*' --numeric-owner \
  && ( sed -i -e 's/#rc_sys=""/rc_sys="docker"/g' etc/rc.conf 2>/dev/null || true ) \
  && echo 'UTC' > etc/timezone \
  && rm ${STAGE3}.asc ${STAGE3}.CONTENTS.gz ${STAGE3}

FROM scratch

WORKDIR /
COPY --from=builder /gentoo/ /


# NOTE(vadorovsky): Rust binaries shipped with rustup are dynamically linked
# against libgcc_s, but libunwind can be used as a drop-in replacement. In
# fact, we should lobby for linking libunwind statically in rustup builds to
# avoid the unnecessary dependency on GCC. :)
#
# https://github.com/rust-lang/rustup/issues/2213#issuecomment-1872985749
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
  && ln -s /usr/lib/libunwind.so.1 /usr/lib/libgcc_s.so.1 \
  && ln -s /usr/lib/libunwind.so /usr/lib/libgcc_s.so

ENV PATH=/root/.cargo/bin:$PATH
CMD ["/bin/bash"]
