# Vortexine's OS 4.5.3

## Arreglado

### Laptops con gráficos Intel viejos (Celeron N30xx, Pentium N3700/N3710 y anteriores)

La 4.5.2 evitaba que un fondo de pantalla trabara el sistema en PCs con gráficos Intel a las que les faltaba el
driver de video moderno (iHD). Pero en los procesadores Intel más viejos (los Atom de la familia Braswell /
Cherry Trail y todo lo anterior a Broadwell) iHD no funciona aunque esté instalado, y el sistema seguía usando
el driver viejo (i965) que falla en esos equipos.

- Ahora el shell revisa el modelo de tu gráfica Intel. Si iHD no la soporta, los fondos de video se decodifican
  con el procesador, igual que cuando falta el driver.
- En las gráficas Intel modernas (Broadwell en adelante) y en AMD y NVIDIA no cambia nada.

### Guías de instalación

- La guía fácil y la de junto a Windows ya dicen que con la ISO 4.5.2 hay que **apagar Secure Boot** (esa ISO no
  viene firmada), con el aviso de BitLocker si conservas Windows.
- **¿Tu USB es de 8 GB?** La guía explica cómo usar la ISO 1.6.3 (4.3 GB) y actualizar al terminar.

## Para actualizar

```
vortex actualizar
```

Al terminar, el shell se reinicia solo.
