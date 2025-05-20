-- AlterTable
ALTER TABLE "displays" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "object_types" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "objects" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "packages" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- CreateTable
CREATE TABLE "blacklist_struct" (
    "id" SERIAL NOT NULL,
    "object_type" TEXT NOT NULL
);

-- CreateIndex
CREATE UNIQUE INDEX "blacklist_struct_object_type_key" ON "blacklist_struct"("object_type");

-- CreateIndex
CREATE INDEX "blacklist_struct_object_type_idx" ON "blacklist_struct"("object_type");
