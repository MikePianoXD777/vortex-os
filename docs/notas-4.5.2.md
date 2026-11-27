# Vortexine's OS 4.5.2

## Arreglado

### Laptops y PCs con gráficos Intel (Celeron y parecidos)

En algunas PCs con gráficos Intel, poner un fondo de pantalla podía trabar todo el sistema. La causa: a las
instalaciones viejas les faltaba el driver de video moderno de Intel (iHD) y el sistema usaba el viejo
(i965), que en esos equipos falla.

- `vortex actualizar` ahora instala el driver iHD (`intel-media-driver`) si tu PC tiene gráficos Intel.
- Mientras no lo tengas, los fondos de video se decodifican con el procesador, sin tocar el driver viejo.
- La ISO nueva ya lo trae de fábrica.

## ISO nueva

Hay ISO nueva de la 4.5.2 (5 partes, USB de 16 GB): [guía de instalación](guia-instalar.md).

**¿No tienes USB?** Instálalo con esta guía: [Instalar junto a Windows sin USB](guia-sin-usb.md).

## Para actualizar

```
vortex actualizar
```

Al terminar, el shell se reinicia solo.
