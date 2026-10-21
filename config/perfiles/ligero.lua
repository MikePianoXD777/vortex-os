-- Perfil «ligero»: sin desenfoque, sin sombras, sin animaciones de ventanas.
-- Para PCs con poca RAM, gráficos integrados viejos o para exprimir FPS.
return {
    animaciones = false,
    vars = {
        blurEnabled         = false,
        shadowEnabled       = false,
        windowOpacity       = 1.0,
        windowGapsIn        = 3,
        windowGapsOut       = 6,
        singleWindowGapsOut = 6,
        workspaceGaps       = 10,
    },
}
